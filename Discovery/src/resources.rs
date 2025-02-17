/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use chrono::{DateTime, Utc};
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
use kube::api::{Resource, ResourceExt};
use regex::Regex;
use serde::Serialize;

use crate::graph::{
    ConfigMapItem, ContainerItem, CronJobItem, DaemonSetItem, DeploymentItem, Graph,
    InitContainerItem, ItemType, JobItem, NamespaceItem, NodeItem, PersistentVolumeClaimItem,
    PersistentVolumeItem, PodItem, Relation, RelationType, SecretItem, ServiceItem,
    StatefulSetItem, StorageClassItem,
};
use crate::state::State;

pub type ResourcesAll = Vec<KubernetesResource>;
pub enum KubernetesResource {
    Node(Node),
    Namespace(Namespace),
    Pod(Pod),
    Service(Service),
    Deployment(Deployment),
    DaemonSet(DaemonSet),
    CronJob(CronJob),
    Job(Job),
    StatefulSet(StatefulSet),
    ReplicaSet(ReplicaSet),
    EndpointSlice(EndpointSlice),
    StorageClass(StorageClass),
    PersistentVolume(PersistentVolume),
    PersistentVolumeClaim(PersistentVolumeClaim),
    ConfigMap(ConfigMap),
    Secret(Secret),
}

impl KubernetesResource {
    pub fn as_endpointslice(&self) -> Option<&EndpointSlice> {
        match self {
            Self::EndpointSlice(eps) => Some(eps),
            _ => None,
        }
    }
    pub fn as_pod(&self) -> Option<&Pod> {
        match self {
            Self::Pod(pod) => Some(pod),
            _ => None,
        }
    }
    pub fn as_replicaset(&self) -> Option<&ReplicaSet> {
        match self {
            Self::ReplicaSet(replicaset) => Some(replicaset),
            _ => None,
        }
    }
    pub fn as_configmap(&self) -> Option<&ConfigMap> {
        match self {
            Self::ConfigMap(config_map) => Some(config_map),
            _ => None,
        }
    }
    pub fn as_secret(&self) -> Option<&Secret> {
        match self {
            Self::Secret(secret) => Some(secret),
            _ => None,
        }
    }
}

fn get_key(api_version: String, kind: String, namespace: String, name: String) -> String {
    format!("{api_version}_{kind}_{namespace}_{name}")
}
const NO_NAMESPACE: &str = "nonamespace";

pub trait ItemExt: ResourceExt<DynamicType = ()> + Sized {
    fn get_namespace(&self) -> String {
        self.namespace().unwrap_or(NO_NAMESPACE.to_string())
    }
    fn get_namespace_item_id(&self, state: &mut State) -> String {
        state.get_item_id(&get_key(
            Namespace::api_version(&()).to_string(),
            Namespace::kind(&()).to_string(),
            NO_NAMESPACE.to_string(),
            self.get_namespace(),
        ))
    }
    fn get_key(&self) -> String {
        get_key(
            Self::api_version(&()).to_string(),
            Self::kind(&()).to_string(),
            self.get_namespace(),
            self.name_any(),
        )
    }
    fn set_resource(&self, resources: &mut ResourcesAll);
    fn set_items_and_relations(
        &self,
        _cluster_item_id: &str,
        _state: &mut State,
        _resources: &ResourcesAll,
        _graph: &mut Graph,
    ) {
    }
    fn is_active_resource(&self, _resources: &ResourcesAll) -> bool {
        true
    }
}

impl ItemExt for Node {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::Node(Node {
            metadata: self.metadata.clone(),
            spec: self.spec.clone(),
            status: self.status.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        _resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let node_item_id = state.get_item_id(&self.get_key());
        graph.add_item(
            node_item_id.clone(),
            ItemType::Node(NodeItem::new(cluster_item_id.to_string(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &node_item_id)),
            RelationType::ClusterNode(Relation::new(
                cluster_item_id.to_string(),
                node_item_id.clone(),
                self,
            )),
        );
    }
}

impl ItemExt for Namespace {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::Namespace(Namespace {
            metadata: self.metadata.clone(),
            spec: self.spec.clone(),
            status: self.status.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        _resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let namespace_item_id = state.get_item_id(&self.get_key());
        graph.add_item(
            namespace_item_id.clone(),
            ItemType::Namespace(NamespaceItem::new(cluster_item_id.to_string(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &namespace_item_id)),
            RelationType::ClusterNamespace(Relation::new(
                cluster_item_id.to_string(),
                namespace_item_id.clone(),
                self,
            )),
        );
    }
}

impl ItemExt for Pod {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::Pod(Pod {
            metadata: self.metadata.clone(),
            spec: self.spec.clone(),
            status: self.status.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let pod_key = self.get_key();
        let pod_item_id = state.get_item_id(&pod_key);
        let namespace_item_id = self.get_namespace_item_id(state);
        graph.add_item(
            pod_item_id.clone(),
            ItemType::Pod(PodItem::new(namespace_item_id.clone(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &pod_item_id)),
            RelationType::ClusterPod(Relation::new(
                cluster_item_id.to_string(),
                pod_item_id.clone(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &namespace_item_id, &pod_item_id)),
            RelationType::NamespacePod(Relation::new(
                namespace_item_id.to_string(),
                pod_item_id.clone(),
                self,
            )),
        );
        if let Some(pod_spec) = self.spec.as_ref() {
            pod_spec.containers.iter().for_each(|container| {
                let container_item_id =
                    state.get_item_id(&format!("{}_{}", pod_key, container.name));
                graph.add_item(
                    container_item_id.clone(),
                    ItemType::Container(ContainerItem::new(
                        pod_item_id.clone(),
                        container.name.clone(),
                        container.image.clone(),
                    )),
                );
                graph.add_relation(
                    state.get_item_id(&format!("{}_{}", &pod_item_id, &container_item_id)),
                    RelationType::PodContainer(Relation::new(
                        pod_item_id.clone(),
                        container_item_id.clone(),
                        self,
                    )),
                );
            });
            if let Some(init_containers) = &pod_spec.init_containers {
                init_containers.iter().for_each(|container| {
                    let container_item_id =
                        state.get_item_id(&format!("{}_{}", pod_key, container.name));
                    graph.add_item(
                        container_item_id.clone(),
                        ItemType::InitContainer(InitContainerItem::new(
                            pod_item_id.clone(),
                            container.name.clone(),
                            container.image.clone(),
                        )),
                    );
                    graph.add_relation(
                        state.get_item_id(&format!("{}_{}", &pod_item_id, &container_item_id)),
                        RelationType::PodInitContainer(Relation::new(
                            pod_item_id.clone(),
                            container_item_id.clone(),
                            self,
                        )),
                    );
                });
            }
            if let Some(node_name) = &pod_spec.node_name {
                let node_item_id = state.get_item_id(&get_key(
                    Node::api_version(&()).to_string(),
                    Node::kind(&()).to_string(),
                    NO_NAMESPACE.to_string(),
                    node_name.to_string(),
                ));
                graph.add_relation(
                    state.get_item_id(&format!("{}_{}", &node_item_id, &pod_item_id)),
                    RelationType::NodePod(Relation::new(
                        node_item_id.clone(),
                        pod_item_id.clone(),
                        self,
                    )),
                );
            }
            if let Some(volumes) = &pod_spec.volumes {
                volumes.iter().for_each(|volume| {
                    if let Some(persistent_volume_claim) = &volume.persistent_volume_claim {
                        let persistent_volume_claim_item_id = state.get_item_id(&get_key(
                            PersistentVolumeClaim::api_version(&()).to_string(),
                            PersistentVolumeClaim::kind(&()).to_string(),
                            self.get_namespace(),
                            persistent_volume_claim.claim_name.clone(),
                        ));
                        graph.add_relation(
                            state.get_item_id(&format!(
                                "{}_{}",
                                &pod_item_id, &persistent_volume_claim_item_id
                            )),
                            RelationType::PodPersistentVolumeClaim(Relation::new(
                                pod_item_id.clone(),
                                persistent_volume_claim_item_id.clone(),
                                self,
                            )),
                        );
                    }
                    if let Some(config_map) = &volume.config_map {
                        if let Some(name) = &config_map.name {
                            let exist = resources
                                .iter()
                                .filter_map(|resource| resource.as_configmap())
                                .any(|config_map| {
                                    &config_map.name_any() == name
                                        && config_map.namespace() == self.namespace()
                                });
                            if exist {
                                let config_map_item_id = state.get_item_id(&get_key(
                                    ConfigMap::api_version(&()).to_string(),
                                    ConfigMap::kind(&()).to_string(),
                                    self.get_namespace(),
                                    name.clone(),
                                ));
                                graph.add_relation(
                                    state.get_item_id(&format!(
                                        "{}_{}",
                                        &pod_item_id, &config_map_item_id
                                    )),
                                    RelationType::PodConfigmap(Relation::new(
                                        pod_item_id.clone(),
                                        config_map_item_id.clone(),
                                        self,
                                    )),
                                );
                            }
                        }
                    }
                    if let Some(secret) = &volume.secret {
                        if let Some(name) = &secret.secret_name {
                            let exist = resources
                                .iter()
                                .filter_map(|resource| resource.as_secret())
                                .any(|secret| {
                                    &secret.name_any() == name
                                        && secret.namespace() == self.namespace()
                                });
                            if exist {
                                let secret_id = state.get_item_id(&get_key(
                                    Secret::api_version(&()).to_string(),
                                    Secret::kind(&()).to_string(),
                                    self.get_namespace(),
                                    name.clone(),
                                ));
                                graph.add_relation(
                                    state.get_item_id(&format!("{}_{}", &pod_item_id, &secret_id)),
                                    RelationType::PodSecret(Relation::new(
                                        pod_item_id.clone(),
                                        secret_id.clone(),
                                        self,
                                    )),
                                );
                            }
                        }
                    }
                })
            }
        };
    }

    fn is_active_resource(&self, resources: &ResourcesAll) -> bool {
        let replicaset_owner_reference = self
            .owner_references()
            .iter()
            .find(|owner_reference| owner_reference.kind == ReplicaSet::kind(&()));
        if let Some(replicaset_owner_reference) = replicaset_owner_reference {
            let deployment_owner_reference = resources
                .iter()
                .filter_map(|resource| resource.as_replicaset())
                .filter(|&resource| {
                    resource.uid() == Some(replicaset_owner_reference.uid.to_string())
                })
                .flat_map(|resource| resource.owner_references())
                .find(|owner_reference| owner_reference.kind == Deployment::kind(&()));
            if let Some(deployment_owner_reference) = deployment_owner_reference {
                let latest_replicaset = resources
                    .iter()
                    .filter_map(|resource| resource.as_replicaset())
                    .filter(|resource| {
                        resource.owner_references().iter().any(|owner_reference| {
                            owner_reference.uid == deployment_owner_reference.uid
                        })
                    })
                    .max_by_key(|replicaset| {
                        replicaset
                            .annotations()
                            .get("deployment.kubernetes.io/revision")
                            .unwrap_or(&"0".to_string())
                            .parse::<i32>()
                            .unwrap_or(0)
                    });
                if let Some(latest_replicaset) = latest_replicaset {
                    if latest_replicaset.uid().as_ref() != Some(&replicaset_owner_reference.uid) {
                        return false;
                    } else {
                        let number_of_replicas =
                            latest_replicaset.spec.as_ref().map_or(0, |replicaset_ref| {
                                replicaset_ref.replicas.map_or(0, |replicas| replicas)
                            });
                        if number_of_replicas > 0 {
                            let mut latest_pods = resources
                                .iter()
                                .filter_map(|resource| resource.as_pod())
                                .filter(|pod| {
                                    pod.owner_references().iter().any(|pod_owner_reference| {
                                        Some(pod_owner_reference.uid.to_string())
                                            == latest_replicaset.uid()
                                    })
                                })
                                .collect::<Vec<&Pod>>();
                            latest_pods.sort_by_key(|pod| &pod.metadata.creation_timestamp);
                            let latest_pods = latest_pods
                                .into_iter()
                                .rev()
                                .take(number_of_replicas as usize)
                                .collect::<Vec<&Pod>>();
                            let mut is_in_latest_pods = false;
                            for pod in latest_pods {
                                if pod.uid() == self.uid() {
                                    is_in_latest_pods = true
                                }
                            }
                            if !is_in_latest_pods {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        true
    }
}

impl ItemExt for Service {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::Service(Service {
            metadata: self.metadata.clone(),
            spec: self.spec.clone(),
            status: self.status.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let service_item_id = state.get_item_id(&self.get_key());
        let namespace_item_id = self.get_namespace_item_id(state);
        graph.add_item(
            service_item_id.clone(),
            ItemType::Service(ServiceItem::new(namespace_item_id.clone(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &service_item_id)),
            RelationType::ClusterService(Relation::new(
                cluster_item_id.to_string(),
                service_item_id.clone(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &namespace_item_id, &service_item_id)),
            RelationType::NamespaceService(Relation::new(
                namespace_item_id.to_string(),
                service_item_id.clone(),
                self,
            )),
        );
        if let Some(service_uid) = self.uid() {
            resources
                .iter()
                .filter_map(|resource| resource.as_endpointslice())
                .filter(|endpointslice| {
                    endpointslice
                        .owner_references()
                        .iter()
                        .any(|owner_reference| owner_reference.uid == service_uid)
                })
                .flat_map(|endpointslice| &endpointslice.endpoints)
                .filter_map(|endpoint| endpoint.target_ref.as_ref())
                .filter(|object_reference| {
                    object_reference.kind == Some(Pod::kind(&()).to_string())
                        && object_reference.name.is_some()
                })
                .filter(|pod_object_reference| {
                    !resources
                        .iter()
                        .filter_map(|resource| resource.as_pod())
                        .filter(|pod| pod.uid() == pod_object_reference.uid)
                        .collect::<Vec<&Pod>>()
                        .is_empty()
                })
                .for_each(|pod_object_reference| {
                    let pod_item_id = state.get_item_id(&get_key(
                        Pod::api_version(&()).to_string(),
                        pod_object_reference.kind.as_ref().unwrap().clone(),
                        pod_object_reference
                            .namespace
                            .as_ref()
                            .unwrap_or(&NO_NAMESPACE.to_string())
                            .clone(),
                        pod_object_reference.name.as_ref().unwrap().clone(),
                    ));
                    graph.add_relation(
                        state
                            .get_item_id(&format!("{}_{}", &service_item_id, &pod_item_id).clone()),
                        RelationType::ServicePod(Relation::new(
                            service_item_id.clone(),
                            pod_item_id.clone(),
                            self,
                        )),
                    );
                })
        }
    }
}

impl ItemExt for Deployment {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::Deployment(Deployment {
            metadata: self.metadata.clone(),
            spec: self.spec.clone(),
            status: self.status.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let deployment_item_id = state.get_item_id(&self.get_key());
        let namespace_item_id = self.get_namespace_item_id(state);
        graph.add_item(
            deployment_item_id.clone(),
            ItemType::Deployment(DeploymentItem::new(namespace_item_id.clone(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &deployment_item_id)),
            RelationType::ClusterDeployment(Relation::new(
                cluster_item_id.to_string(),
                deployment_item_id.clone(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &namespace_item_id, &deployment_item_id)),
            RelationType::NamespaceDeployment(Relation::new(
                namespace_item_id.to_string(),
                deployment_item_id.clone(),
                self,
            )),
        );
        if let Some(deployment_uid) = self.uid() {
            let deployment_replicaset_uids = resources
                .iter()
                .filter_map(|resource| resource.as_replicaset())
                .filter(|replicaset| {
                    !replicaset
                        .owner_references()
                        .iter()
                        .filter(|owner_reference| owner_reference.uid == deployment_uid)
                        .collect::<Vec<_>>()
                        .is_empty()
                })
                .filter_map(|replicaset| replicaset.uid())
                .collect::<Vec<_>>();
            resources
                .iter()
                .filter_map(|resource| resource.as_pod())
                .filter(|pod| {
                    !pod.owner_references()
                        .iter()
                        .filter(|owner_reference| {
                            deployment_replicaset_uids.contains(&owner_reference.uid)
                        })
                        .collect::<Vec<_>>()
                        .is_empty()
                })
                .for_each(|pod| {
                    let pod_item_id = match state.get_existing_item_id(&pod.get_key()) {
                        Some(pod_item_id) => {
                            pod.set_items_and_relations(cluster_item_id, state, resources, graph);
                            pod_item_id
                        }
                        None => state.get_item_id(&pod.get_key()),
                    };
                    graph.add_relation(
                        state.get_item_id(
                            &format!("{}_{}", &deployment_item_id, &pod_item_id).clone(),
                        ),
                        RelationType::DeploymentPod(Relation::new(
                            deployment_item_id.clone(),
                            pod_item_id.clone(),
                            self,
                        )),
                    );
                });
        };
    }
}

impl ItemExt for StatefulSet {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::StatefulSet(StatefulSet {
            metadata: self.metadata.clone(),
            spec: self.spec.clone(),
            status: self.status.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let statefulset_item_id = state.get_item_id(&self.get_key());
        let namespace_item_id = self.get_namespace_item_id(state);
        graph.add_item(
            statefulset_item_id.clone(),
            ItemType::StatefulSet(StatefulSetItem::new(namespace_item_id.clone(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &statefulset_item_id)),
            RelationType::ClusterStatefulSet(Relation::new(
                cluster_item_id.to_string(),
                statefulset_item_id.clone(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &namespace_item_id, &statefulset_item_id)),
            RelationType::NamespaceStatefulSet(Relation::new(
                namespace_item_id.to_string(),
                statefulset_item_id.clone(),
                self,
            )),
        );
        if let Some(statefulset_uid) = self.uid() {
            resources
                .iter()
                .filter_map(|resource| resource.as_pod())
                .filter(|pod| {
                    !pod.owner_references()
                        .iter()
                        .filter(|owner_reference| owner_reference.uid == statefulset_uid)
                        .collect::<Vec<_>>()
                        .is_empty()
                })
                .for_each(|pod| {
                    let pod_item_id = state.get_item_id(&pod.get_key());
                    graph.add_relation(
                        state.get_item_id(
                            &format!("{}_{}", &statefulset_item_id, &pod_item_id).clone(),
                        ),
                        RelationType::StatefulsetPod(Relation::new(
                            statefulset_item_id.clone(),
                            pod_item_id.clone(),
                            self,
                        )),
                    );
                });
        };
    }
}

impl ItemExt for DaemonSet {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::DaemonSet(DaemonSet {
            metadata: self.metadata.clone(),
            spec: self.spec.clone(),
            status: self.status.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let daemon_set_item_id = state.get_item_id(&self.get_key());
        let namespace_item_id = self.get_namespace_item_id(state);
        graph.add_item(
            daemon_set_item_id.clone(),
            ItemType::DaemonSet(DaemonSetItem::new(namespace_item_id.clone(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &daemon_set_item_id)),
            RelationType::ClusterDaemonSet(Relation::new(
                cluster_item_id.to_string(),
                daemon_set_item_id.clone(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &namespace_item_id, &daemon_set_item_id)),
            RelationType::NamespaceDaemonSet(Relation::new(
                namespace_item_id.to_string(),
                daemon_set_item_id.clone(),
                self,
            )),
        );
        if let Some(daemon_set_uid) = self.uid() {
            resources
                .iter()
                .filter_map(|resource| resource.as_pod())
                .filter(|pod| {
                    !pod.owner_references()
                        .iter()
                        .filter(|owner_reference| owner_reference.uid == daemon_set_uid)
                        .collect::<Vec<_>>()
                        .is_empty()
                })
                .for_each(|pod| {
                    let pod_item_id = state.get_item_id(&pod.get_key());
                    graph.add_relation(
                        state.get_item_id(
                            &format!("{}_{}", &daemon_set_item_id, &pod_item_id).clone(),
                        ),
                        RelationType::DaemonSetPod(Relation::new(
                            daemon_set_item_id.clone(),
                            pod_item_id.clone(),
                            self,
                        )),
                    );
                });
        };
    }
}

impl ItemExt for CronJob {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::CronJob(CronJob {
            metadata: self.metadata.clone(),
            spec: self.spec.clone(),
            status: self.status.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let cron_job_item_id = state.get_item_id(&self.get_key());
        let namespace_item_id = self.get_namespace_item_id(state);
        graph.add_item(
            cron_job_item_id.clone(),
            ItemType::CronJob(CronJobItem::new(namespace_item_id.clone(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &cron_job_item_id)),
            RelationType::ClusterCronJob(Relation::new(
                cluster_item_id.to_string(),
                cron_job_item_id.clone(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &namespace_item_id, &cron_job_item_id)),
            RelationType::NamespaceCronJob(Relation::new(
                namespace_item_id.to_string(),
                cron_job_item_id.clone(),
                self,
            )),
        );
        if let Some(cron_job_uid) = self.uid() {
            resources
                .iter()
                .filter_map(|resource| resource.as_pod())
                .filter(|pod| {
                    !pod.owner_references()
                        .iter()
                        .filter(|owner_reference| owner_reference.uid == cron_job_uid)
                        .collect::<Vec<_>>()
                        .is_empty()
                })
                .for_each(|pod| {
                    let pod_item_id = state.get_item_id(&pod.get_key());
                    graph.add_relation(
                        state.get_item_id(
                            &format!("{}_{}", &cron_job_item_id, &pod_item_id).clone(),
                        ),
                        RelationType::CronJobPod(Relation::new(
                            cron_job_item_id.clone(),
                            pod_item_id.clone(),
                            self,
                        )),
                    );
                });
        };
    }
}

impl ItemExt for Job {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::Job(Job {
            metadata: self.metadata.clone(),
            spec: self.spec.clone(),
            status: self.status.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let job_item_id = state.get_item_id(&self.get_key());
        let namespace_item_id = self.get_namespace_item_id(state);
        graph.add_item(
            job_item_id.clone(),
            ItemType::Job(JobItem::new(namespace_item_id.clone(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &job_item_id)),
            RelationType::ClusterJob(Relation::new(
                cluster_item_id.to_string(),
                job_item_id.clone(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &namespace_item_id, &job_item_id)),
            RelationType::NamespaceJob(Relation::new(
                namespace_item_id.to_string(),
                job_item_id.clone(),
                self,
            )),
        );
        if let Some(job_uid) = self.uid() {
            resources
                .iter()
                .filter_map(|resource| resource.as_pod())
                .filter(|pod| {
                    !pod.owner_references()
                        .iter()
                        .filter(|owner_reference| owner_reference.uid == job_uid)
                        .collect::<Vec<_>>()
                        .is_empty()
                })
                .for_each(|pod| {
                    let pod_item_id = state.get_item_id(&pod.get_key());
                    graph.add_relation(
                        state.get_item_id(&format!("{}_{}", &job_item_id, &pod_item_id).clone()),
                        RelationType::JobPod(Relation::new(
                            job_item_id.clone(),
                            pod_item_id.clone(),
                            self,
                        )),
                    );
                });
        };
    }
}

impl ItemExt for ReplicaSet {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::ReplicaSet(ReplicaSet {
            metadata: self.metadata.clone(),
            spec: self.spec.clone(),
            status: self.status.clone(),
        }));
    }
    fn is_active_resource(&self, resources: &ResourcesAll) -> bool {
        let deployment_owner_reference = self
            .owner_references()
            .iter()
            .find(|owner_reference| owner_reference.kind == Deployment::kind(&()));
        if let Some(deployment_owner_reference) = deployment_owner_reference {
            let latest_replicaset = resources
                .iter()
                .filter_map(|resource| resource.as_replicaset())
                .filter(|resource| {
                    resource.owner_references().iter().any(|owner_reference| {
                        owner_reference.uid == deployment_owner_reference.uid
                    })
                })
                .max_by_key(|replicaset| {
                    replicaset
                        .annotations()
                        .get("deployment.kubernetes.io/revision")
                        .unwrap_or(&"0".to_string())
                        .parse::<i32>()
                        .unwrap_or(0)
                });

            if let Some(latest_replicaset) = latest_replicaset {
                if latest_replicaset.uid() != self.uid() {
                    return false;
                }
            }
        }
        true
    }
}

impl ItemExt for EndpointSlice {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::EndpointSlice(EndpointSlice {
            metadata: self.metadata.clone(),
            address_type: self.address_type.clone(),
            endpoints: self.endpoints.clone(),
            ports: self.ports.clone(),
        }));
    }
}

impl ItemExt for StorageClass {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::StorageClass(StorageClass {
            allow_volume_expansion: self.allow_volume_expansion,
            allowed_topologies: self.allowed_topologies.clone(),
            metadata: self.metadata.clone(),
            mount_options: self.mount_options.clone(),
            parameters: self.parameters.clone(),
            provisioner: self.provisioner.clone(),
            reclaim_policy: self.reclaim_policy.clone(),
            volume_binding_mode: self.volume_binding_mode.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        _resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let storage_class_item_id = state.get_item_id(&self.get_key());
        graph.add_item(
            storage_class_item_id.clone(),
            ItemType::StorageClass(StorageClassItem::new(cluster_item_id.to_string(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &storage_class_item_id)),
            RelationType::ClusterStorageClass(Relation::new(
                cluster_item_id.to_string(),
                storage_class_item_id.clone(),
                self,
            )),
        );
    }
}

impl ItemExt for PersistentVolume {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::PersistentVolume(PersistentVolume {
            metadata: self.metadata.clone(),
            spec: self.spec.clone(),
            status: self.status.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        _resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let persistent_volume_item_id = state.get_item_id(&self.get_key());
        graph.add_item(
            persistent_volume_item_id.clone(),
            ItemType::PersistentVolume(PersistentVolumeItem::new(
                cluster_item_id.to_string(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!(
                "{}_{}",
                &cluster_item_id, &persistent_volume_item_id
            )),
            RelationType::ClusterPersistentVolume(Relation::new(
                cluster_item_id.to_string(),
                persistent_volume_item_id.clone(),
                self,
            )),
        );
        if let Some(spec) = &self.spec {
            if let Some(storage_class_name) = &spec.storage_class_name {
                let storage_class_item_id = state.get_item_id(&get_key(
                    StorageClass::api_version(&()).to_string(),
                    StorageClass::kind(&()).to_string(),
                    NO_NAMESPACE.to_string(),
                    storage_class_name.clone(),
                ));
                graph.add_relation(
                    state.get_item_id(&format!(
                        "{}_{}",
                        &storage_class_item_id, &persistent_volume_item_id
                    )),
                    RelationType::StorageClassPersistentVolume(Relation::new(
                        storage_class_item_id.clone(),
                        persistent_volume_item_id.clone(),
                        self,
                    )),
                );
            }
        }
    }
}

impl ItemExt for PersistentVolumeClaim {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::PersistentVolumeClaim(
            PersistentVolumeClaim {
                metadata: self.metadata.clone(),
                spec: self.spec.clone(),
                status: self.status.clone(),
            },
        ));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        _resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let persistent_volume_claim_item_id = state.get_item_id(&self.get_key());
        let namespace_item_id = self.get_namespace_item_id(state);
        graph.add_item(
            persistent_volume_claim_item_id.clone(),
            ItemType::PersistentVolumeClaim(PersistentVolumeClaimItem::new(
                namespace_item_id.clone(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!(
                "{}_{}",
                &cluster_item_id, &persistent_volume_claim_item_id
            )),
            RelationType::ClusterPersistentVolumeClaim(Relation::new(
                cluster_item_id.to_string(),
                persistent_volume_claim_item_id.clone(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!(
                "{}_{}",
                &namespace_item_id, &persistent_volume_claim_item_id
            )),
            RelationType::NamespacePersistentVolumeClaim(Relation::new(
                namespace_item_id.to_string(),
                persistent_volume_claim_item_id.clone(),
                self,
            )),
        );
        if let Some(spec) = &self.spec {
            if let Some(persistent_volume_name) = &spec.volume_name {
                let persistent_volume_item_id = state.get_item_id(&get_key(
                    PersistentVolume::api_version(&()).to_string(),
                    PersistentVolume::kind(&()).to_string(),
                    NO_NAMESPACE.to_string(),
                    persistent_volume_name.clone(),
                ));
                graph.add_relation(
                    state.get_item_id(&format!(
                        "{}_{}",
                        &persistent_volume_claim_item_id, &persistent_volume_item_id
                    )),
                    RelationType::PersistentVolumeClaimPersistentVolume(Relation::new(
                        persistent_volume_claim_item_id.clone(),
                        persistent_volume_item_id.clone(),
                        self,
                    )),
                );
            }
        }
    }
}

impl ItemExt for ConfigMap {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::ConfigMap(ConfigMap {
            binary_data: self.binary_data.clone(),
            data: self.data.clone(),
            immutable: self.immutable,
            metadata: self.metadata.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        _resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let configmap_item_id = state.get_item_id(&self.get_key());
        let namespace_item_id = self.get_namespace_item_id(state);
        graph.add_item(
            configmap_item_id.clone(),
            ItemType::ConfigMap(ConfigMapItem::new(namespace_item_id.clone(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &configmap_item_id)),
            RelationType::ClusterConfigMap(Relation::new(
                cluster_item_id.to_string(),
                configmap_item_id.clone(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &namespace_item_id, &configmap_item_id)),
            RelationType::NamespaceConfigMap(Relation::new(
                namespace_item_id.to_string(),
                configmap_item_id.clone(),
                self,
            )),
        );
    }
}

impl ItemExt for Secret {
    fn set_resource(&self, resources: &mut ResourcesAll) {
        resources.push(KubernetesResource::Secret(Secret {
            string_data: self.string_data.clone(),
            data: self.data.clone(),
            immutable: self.immutable,
            metadata: self.metadata.clone(),
            type_: self.type_.clone(),
        }));
    }
    fn set_items_and_relations(
        &self,
        cluster_item_id: &str,
        state: &mut State,
        _resources: &ResourcesAll,
        graph: &mut Graph,
    ) {
        let secret_item_id = state.get_item_id(&self.get_key());
        let namespace_item_id = self.get_namespace_item_id(state);
        graph.add_item(
            secret_item_id.clone(),
            ItemType::Secret(SecretItem::new(namespace_item_id.clone(), self)),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &cluster_item_id, &secret_item_id)),
            RelationType::ClusterSecret(Relation::new(
                cluster_item_id.to_string(),
                secret_item_id.clone(),
                self,
            )),
        );
        graph.add_relation(
            state.get_item_id(&format!("{}_{}", &namespace_item_id, &secret_item_id)),
            RelationType::NamespaceSecret(Relation::new(
                namespace_item_id.to_string(),
                secret_item_id.clone(),
                self,
            )),
        );
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum EventDocument {
    Detached(DetachedEvent),
    Item(ItemEvent),
    //Relation(RelationEvent),
}

#[derive(Debug, Serialize)]
pub struct DetachedEvent {
    timestamp: DateTime<Utc>,
    cluster_name: String,
    #[serde(flatten)]
    log: Event,
}

#[derive(Debug, Serialize)]
pub struct ItemEvent {
    timestamp: DateTime<Utc>,
    cluster_name: String,
    item_id: String,
    item_type: String,
    related_item_id: Option<String>,
    related_item_type: Option<String>,
    #[serde(flatten)]
    log: Event,
}
// #[derive(Debug, Serialize)]
// pub struct RelationEvent {
//     timestamp: DateTime<Utc>,
//     cluster_name: String,
//     relation_id: String,
//     relation_type: String,
//     #[serde(flatten)]
//     log: Event,
// }

pub trait EventExt: ResourceExt<DynamicType = ()> + Sized {
    fn to_document(&self, cluster_name: &str, state: &State) -> EventDocument;
}

impl EventExt for Event {
    fn to_document(&self, cluster_name: &str, state: &State) -> EventDocument {
        if let (Some(object), related_object) = (&self.regarding, &self.related) {
            if let (Some(api_version), Some(kind), namespace, Some(name)) = (
                &object.api_version,
                &object.kind,
                object.namespace.clone().unwrap_or(NO_NAMESPACE.to_string()),
                &object.name,
            ) {
                let (related_item_id, related_item_type) =
                    if let Some(related_object) = related_object {
                        if let (Some(api_version), Some(kind), namespace, Some(name)) = (
                            &related_object.api_version,
                            &related_object.kind,
                            related_object
                                .namespace
                                .clone()
                                .unwrap_or(NO_NAMESPACE.to_string()),
                            &related_object.name,
                        ) {
                            let related_item_id = state.get_existing_item_id(&get_key(
                                api_version.to_string(),
                                kind.to_string(),
                                namespace,
                                name.to_string(),
                            ));
                            if let Some(related_item_id) = related_item_id {
                                let mut related_item_type = "kubernetes/".to_string();
                                related_item_type.push_str(&kind.to_lowercase());
                                (Some(related_item_id), Some(related_item_type))
                            } else {
                                (None, None)
                            }
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    };
                let item_id = state.get_existing_item_id(&get_key(
                    api_version.to_string(),
                    kind.to_string(),
                    namespace,
                    name.to_string(),
                ));
                if let Some(item_id) = item_id {
                    let mut item_type = "kubernetes/".to_string();
                    item_type.push_str(&pascal_to_snake_case(kind));
                    return EventDocument::Item(ItemEvent {
                        timestamp: Utc::now(),
                        cluster_name: cluster_name.to_string(),
                        item_id,
                        item_type,
                        related_item_id,
                        related_item_type,
                        log: self.clone(),
                    });
                }
            }
        }
        EventDocument::Detached(DetachedEvent {
            timestamp: Utc::now(),
            cluster_name: cluster_name.to_string(),
            log: self.clone(),
        })
    }
}

fn pascal_to_snake_case(input: &str) -> String {
    let re = Regex::new(r"([a-z])([A-Z])").unwrap();
    let result = re.replace_all(input, "$1_$2");
    result.to_lowercase()
}
