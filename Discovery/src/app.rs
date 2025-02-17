/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::fmt::Debug;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use futures::{future::OptionFuture, prelude::*};
use k8s_openapi::api::{
    apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet},
    batch::v1::{CronJob, Job},
    core::v1::{
        ConfigMap, Namespace, Node, PersistentVolume, PersistentVolumeClaim, Pod, Secret, Service,
    },
    discovery::v1::EndpointSlice,
    events::v1::Event,
    storage::v1::StorageClass,
};
use k8s_openapi::serde::de::DeserializeOwned;
use kube::{
    api::{Api, ListParams, Resource, VersionMatch},
    runtime::{watcher, WatchStreamExt},
    Client,
};
use opensearch::{
    auth::{ClientCertificate, Credentials},
    cert::{Certificate, CertificateValidation},
    http::transport::{SingleNodeConnectionPool, TransportBuilder},
    indices::{IndicesCreateParts, IndicesExistsParts},
    OpenSearch,
};
use reqwest::Url;
use serde_json::json;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{interval, MissedTickBehavior};

use crate::{
    auth::{AuthConfig, Authenticator},
    error::{Error, Result},
    graph::{ClusterItem, Graph, ItemType},
    resources::{EventExt, ItemExt, KubernetesResource, ResourcesAll},
    state::State,
};

const K8S_LOG_INDEX: &str = "logs-kubernetes";

/// K8s Discovery
#[derive(Parser)]
pub struct Config {
    /// Cluster name
    #[clap(short, long, env = "CONTINUOUSC_CLUSTER")]
    cluster: String,
    /// Directory to save state file
    #[clap(
        short,
        long,
        env = "CONTINUOUSC_STATE",
        default_value = "/usr/share/continuousc/state"
    )]
    state: PathBuf,
    /// Update interval. If unset, discovery is run only once.
    #[clap(long, env = "CONTINUOUSC_INTERVAL")]
    interval: Option<u64>,
    /// Url of the Relation Graph Engine.
    #[clap(long, env = "CONTINUOUSC_ENGINE")]
    engine: Option<Url>,
    /// Authentication parameters.
    #[command(flatten)]
    auth: Option<AuthConfig>,
    /// Save events to Opensearch
    #[clap(long, env = "CONTINUOUSC_EVENTS", default_value = "false")]
    events: bool,
    /// Url of Opensearch
    #[clap(
        long,
        env = "CONTINUOUSC_OPENSEARCH_URL",
        required_if_eq("events", "true")
    )]
    opensearch_url: Option<Url>,
    /// authentication cert
    #[clap(long, env = "CONTINUOUSC_AUTH_CERT", required_if_eq("events", "true"))]
    auth_cert: Option<PathBuf>,
    /// auth key path
    #[clap(long, env = "CONTINUOUSC_AUTH_KEY", required_if_eq("events", "true"))]
    auth_key: Option<PathBuf>,
    /// auth ca path
    #[clap(long, env = "CONTINUOUSC_AUTH_CA")]
    auth_ca: Option<PathBuf>,
}

pub struct App {
    config: Config,
    authenticator: Authenticator,
    client: Client,
    state: State,
    resources: ResourcesAll,
    resources_filtered: ResourcesAll,
    graph: Graph,
    engine_client: reqwest::Client,
}

impl App {
    pub async fn new(config: Config) -> Result<App> {
        let mut engine_client = reqwest::Client::builder();

        if let Some(ca_path) = config.auth_ca.as_ref() {
            let ca_data = tokio::fs::read(ca_path).await.map_err(Error::ReadCaCert)?;
            let ca_cert = reqwest::Certificate::from_pem(&ca_data).map_err(Error::LoadCaCert)?;
            engine_client = engine_client
                .tls_built_in_root_certs(false)
                .add_root_certificate(ca_cert);
        }
        let engine_client = engine_client.build().map_err(Error::BuildEngineClient)?;

        Ok(App {
            state: State::get(&config.state).await.unwrap_or_default(),
            authenticator: Authenticator::default(),
            config,
            client: Client::try_default().await?,
            resources: vec![],
            resources_filtered: vec![],
            graph: Graph::new(),
            engine_client,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        if let Some(mins) = self.config.interval {
            let mut events = if self.config.events {
                let (client, stream) = self.get_events_stream().await?;
                Some((client, Box::pin(stream)))
            } else {
                None
            };
            let mut sigterm = signal(SignalKind::terminate())?;
            let mut sigint = signal(SignalKind::interrupt())?;
            let mut interval = interval(Duration::from_secs(mins * 60));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                let next_event: OptionFuture<_> =
                    events.as_mut().map(|(_, events)| events.next()).into();
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = self.run_once().await {
                            log::error!("Discovery failed: {e}; will retry at next interval!");
                        } else {
                            log::info!("Discovery finished successfully")
                        }
                    },
                    Some(Some(event)) = next_event => {
                        self.write_event(&events.as_ref().unwrap().0, event).await;
                    }
                    Some(_) = sigint.recv() => {
                        log::info!("Caught SIGINT; shutting down...");
                        break
                    }
                    Some(_) = sigterm.recv() => {
                        log::info!("Caught SIGTERM; shutting down...");
                        break
                    }
                }
            }
        } else if let Err(e) = self.run_once().await {
            log::error!("Discovery failed: {e}!");
        } else {
            log::info!("Discovery finished successfully")
        }
        Ok(())
    }

    async fn run_once(&mut self) -> Result<()> {
        log::debug!("Running discovery...");
        self.resources.clear();
        self.resources_filtered.clear();
        self.graph.items.clear();
        self.graph.relations.clear();

        let resource_version = Api::<Node>::all(self.client.clone())
            .list_metadata(&ListParams::default())
            .await?
            .metadata
            .resource_version
            .expect("Expected a resource version");

        self.get_resources::<Node>(&resource_version).await?;
        self.get_resources::<Namespace>(&resource_version).await?;
        self.get_resources::<Pod>(&resource_version).await?;
        self.get_resources::<Service>(&resource_version).await?;
        self.get_resources::<Deployment>(&resource_version).await?;
        self.get_resources::<StatefulSet>(&resource_version).await?;
        self.get_resources::<ReplicaSet>(&resource_version).await?;
        self.get_resources::<EndpointSlice>(&resource_version)
            .await?;
        self.get_resources::<StorageClass>(&resource_version)
            .await?;
        self.get_resources::<PersistentVolume>(&resource_version)
            .await?;
        self.get_resources::<PersistentVolumeClaim>(&resource_version)
            .await?;
        self.get_resources::<ConfigMap>(&resource_version).await?;
        self.get_resources::<Secret>(&resource_version).await?;
        self.get_resources::<DaemonSet>(&resource_version).await?;
        self.get_resources::<CronJob>(&resource_version).await?;
        self.get_resources::<Job>(&resource_version).await?;
        self.filter_resources();
        self.create_cluster_item();
        self.get_graph();
        self.state.save(&self.config.state).await?;
        self.graph.save_to_file(&self.config.state).await?;
        if let Some(url) = &self.config.engine {
            self.graph
                .save_to_engine(
                    &self.engine_client,
                    url,
                    &mut self.authenticator,
                    self.config.auth.as_ref(),
                )
                .await?;
        }
        Ok(())
    }

    async fn get_events_stream(
        &self,
    ) -> Result<(
        OpenSearch,
        impl Stream<Item = kube::runtime::watcher::Result<Event>>,
    )> {
        let opensearch_url = self.config.opensearch_url.as_ref().unwrap().clone();
        let connection_pool = SingleNodeConnectionPool::new(opensearch_url);
        let transport_builder = TransportBuilder::new(connection_pool);
        let transport_builder = if let Some(auth_ca) = &self.config.auth_ca {
            let ca = Certificate::from_pem(&tokio::fs::read(auth_ca).await?)?;
            transport_builder.cert_validation(CertificateValidation::Full(ca))
        } else {
            transport_builder
        };
        let transport_builder = if let (Some(auth_cert), Some(auth_key)) =
            (&self.config.auth_cert, &self.config.auth_key)
        {
            let mut cert = tokio::fs::read(auth_cert).await?;
            let mut key = tokio::fs::read(auth_key).await?;
            key.append(&mut cert);
            transport_builder.auth(Credentials::Certificate(ClientCertificate::Pem(key)))
        } else {
            transport_builder
        };
        let transport = transport_builder.build()?;
        let client_opensearch = OpenSearch::new(transport);

        let index_exist = client_opensearch
            .indices()
            .exists(IndicesExistsParts::Index(&[K8S_LOG_INDEX]))
            .send()
            .await?
            .status_code()
            == 200;

        if !index_exist {
            client_opensearch
                .indices()
                .create(IndicesCreateParts::Index(K8S_LOG_INDEX))
                .send()
                .await?;
        }

        Ok((
            client_opensearch,
            watcher(
                Api::<Event>::all(self.client.clone()),
                watcher::Config {
                    bookmarks: true,
                    label_selector: None,
                    field_selector: None,
                    timeout: None,
                    list_semantic: watcher::ListSemantic::Any,
                    initial_list_strategy: watcher::InitialListStrategy::ListWatch,
                    page_size: Some(500),
                },
            )
            .touched_objects()
            .default_backoff(),
        ))
    }

    async fn write_event(
        &self,
        client_opensearch: &OpenSearch,
        event: kube::runtime::watcher::Result<Event>,
    ) {
        match event {
            Ok(event) => {
                let event_document = event.to_document(&self.config.cluster, &self.state);
                match client_opensearch
                    .index(opensearch::IndexParts::Index(K8S_LOG_INDEX))
                    .body(json!(event_document))
                    .send()
                    .await
                {
                    Ok(_) => (),
                    Err(err) => {
                        log::error!("Failed to insert event in opensearch: {err}")
                    }
                };
            }
            Err(err) => {
                log::error!("Failed to watch kubernetes event: {err}")
            }
        }
    }

    pub async fn get_resources<T>(&mut self, resource_version: &str) -> Result<()>
    where
        T: ItemExt + Resource<DynamicType = ()> + Debug + DeserializeOwned + Clone,
    {
        Api::<T>::all(self.client.clone())
            .list(
                &ListParams::default()
                    .at(resource_version)
                    .matching(VersionMatch::Exact),
            )
            .await?
            .items
            .into_iter()
            .for_each(|resource| {
                resource.set_resource(&mut self.resources);
            });

        Ok(())
    }

    pub fn filter_resources(&mut self) {
        self.resources.iter().for_each(|resource| match resource {
            KubernetesResource::Node(node) => {
                if node.is_active_resource(&self.resources) {
                    node.set_resource(&mut self.resources_filtered);
                }
            }
            KubernetesResource::Namespace(namespace) => {
                if namespace.is_active_resource(&self.resources) {
                    namespace.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::Pod(pod) => {
                if pod.is_active_resource(&self.resources) {
                    pod.set_resource(&mut self.resources_filtered);
                }
            }
            KubernetesResource::Service(service) => {
                if service.is_active_resource(&self.resources) {
                    service.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::Deployment(deployment) => {
                if deployment.is_active_resource(&self.resources) {
                    deployment.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::StatefulSet(statefulset) => {
                if statefulset.is_active_resource(&self.resources) {
                    statefulset.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::ReplicaSet(replicaset) => {
                if replicaset.is_active_resource(&self.resources) {
                    replicaset.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::EndpointSlice(endpointslice) => {
                if endpointslice.is_active_resource(&self.resources) {
                    endpointslice.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::StorageClass(storageclass) => {
                if storageclass.is_active_resource(&self.resources) {
                    storageclass.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::PersistentVolume(persistentvolume) => {
                if persistentvolume.is_active_resource(&self.resources) {
                    persistentvolume.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::PersistentVolumeClaim(persistentvolumeclaim) => {
                if persistentvolumeclaim.is_active_resource(&self.resources) {
                    persistentvolumeclaim.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::ConfigMap(configmap) => {
                if configmap.is_active_resource(&self.resources) {
                    configmap.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::Secret(secret) => {
                if secret.is_active_resource(&self.resources) {
                    secret.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::DaemonSet(daemonset) => {
                if daemonset.is_active_resource(&self.resources) {
                    daemonset.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::CronJob(cronjob) => {
                if cronjob.is_active_resource(&self.resources) {
                    cronjob.set_resource(&mut self.resources_filtered);
                };
            }
            KubernetesResource::Job(job) => {
                if job.is_active_resource(&self.resources) {
                    job.set_resource(&mut self.resources_filtered);
                };
            }
        });
    }

    pub fn create_cluster_item(&mut self) {
        let cluster_item_id = self.state.get_item_id(&self.config.cluster);
        self.graph.add_item(
            cluster_item_id.clone(),
            ItemType::Cluster(ClusterItem::new(self.config.cluster.clone())),
        );
    }

    fn get_graph(&mut self) {
        let cluster_item_id = &self.state.get_item_id(&self.config.cluster);
        self.resources_filtered
            .iter()
            .for_each(|resource| match resource {
                KubernetesResource::Node(node) => {
                    node.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::Namespace(namespace) => {
                    namespace.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::Pod(pod) => {
                    pod.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::Service(service) => {
                    service.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::Deployment(deployment) => {
                    deployment.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::StatefulSet(statefulset) => {
                    statefulset.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::StorageClass(storageclass) => {
                    storageclass.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::PersistentVolume(persistentvolume) => {
                    persistentvolume.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::PersistentVolumeClaim(persistentvolumeclaim) => {
                    persistentvolumeclaim.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::ConfigMap(configmap) => {
                    configmap.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::Secret(secret) => {
                    secret.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::DaemonSet(daemon_set) => {
                    daemon_set.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::CronJob(cron_job) => {
                    cron_job.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                KubernetesResource::Job(job) => {
                    job.set_items_and_relations(
                        cluster_item_id,
                        &mut self.state,
                        &self.resources_filtered,
                        &mut self.graph,
                    );
                }
                _ => (),
            });
    }
}
