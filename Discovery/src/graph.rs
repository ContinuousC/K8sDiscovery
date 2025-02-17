/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::path::Path;

use chrono::{DateTime, Utc};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::api::{Resource, ResourceExt};
use reqwest::Url;
use serde::Serialize;
use serde_json::json;
use tokio::fs;

use crate::auth::{AuthConfig, Authenticator};
use crate::error::{Error, Result};

serde_with::with_prefix!(prefix_kubernetes "kubernetes/");
#[derive(Debug, Serialize)]
pub struct Graph {
    pub items: Items,
    pub relations: Relations,
}
impl Graph {
    pub fn new() -> Self {
        Graph {
            items: HashMap::new(),
            relations: HashMap::new(),
        }
    }
    pub fn add_item(&mut self, id: String, item: ItemType) {
        self.items.insert(id, item);
    }
    pub fn add_relation(&mut self, id: String, relation: RelationType) {
        self.relations.insert(id, relation);
    }
    pub async fn save_to_file(&self, path: &Path) -> Result<()> {
        fs::write(path.join("items.json"), serde_json::to_string_pretty(self)?).await?;
        Ok(())
    }
    pub async fn save_to_engine(
        &self,
        client: &reqwest::Client,
        url: &Url,
        authenticator: &mut Authenticator,
        auth_config: Option<&AuthConfig>,
    ) -> Result<()> {
        let cluster_ids = self
            .items
            .iter()
            .filter_map(|(id, item)| matches!(item, ItemType::Cluster(_)).then_some(id))
            .collect::<Vec<_>>();
        log::debug!("Saving elements into relation graph...");
        let mut req = client.put(url.join("items")?);
        if let Some(config) = auth_config {
            let token = authenticator.get_token(config, client).await?;
            req = req.bearer_auth(token);
        } else {
            req = req.header("X-Proxy-Role", "Editor");
        }
        let r = req.json(&json!({
                "domain": {
                    "roots": cluster_ids,
             		"types": {
						"items": ["kubernetes/cluster", "kubernetes/node", "kubernetes/namespace",
                                "kubernetes/pod", "kubernetes/container", "kubernetes/init_container",
                                "kubernetes/service", "kubernetes/deployment", "kubernetes/stateful_set",
                                "kubernetes/storage_class", "kubernetes/persistent_volume",
                                "kubernetes/persistent_volume_claim", "kubernetes/config_map", "kubernetes/secret",
                                "kubernetes/daemon_set", "kubernetes/cron_job", "kubernetes/job"],
						"relations": ["kubernetes/cluster-node", "kubernetes/cluster-namespace", "kubernetes/cluster-pod",
                                    "kubernetes/cluster-service", "kubernetes/cluster-deployment", "kubernetes/cluster-stateful_set",
                                    "kubernetes/cluster-storage_class", "kubernetes/cluster-persistent_volume", "kubernetes/cluster-persistent_volume_claim",
                                    "kubernetes/cluster-config_map", "kubernetes/cluster-secret", "kubernetes/namespace-pod",
                                    "kubernetes/namespace-service", "kubernetes/namespace-deployment", "kubernetes/namespace-stateful_set",
                                    "kubernetes/namespace-persistent_volume_claim", "kubernetes/namespace-config_map", "kubernetes/namespace-secret",
                                    "kubernetes/node-pod", "kubernetes/pod-container", "kubernetes/pod-init_container", "kubernetes/service-pod",
                                    "kubernetes/deployment-pod", "kubernetes/stateful_set-pod", "kubernetes/pod-persistent_volume_claim",
                                    "kubernetes/pod-config_map", "kubernetes/pod-secret", "kubernetes/persistent_volume_claim-persistent_volume",
                                    "kubernetes/storage_class-persistent_volume",
                                    "kubernetes/daemon_set-pod", "kubernetes/cron_job-pod", "kubernetes/job-pod",
                                    "kubernetes/cluster-daemon_set", "kubernetes/cluster-cron_job", "kubernetes/cluster-job",
                                    "kubernetes/namespace-daemon_set", "kubernetes/namespace-cron_job", "kubernetes/namespace-job"]
					}
                },
				"items": self
            }))
			.timeout(std::time::Duration::from_secs(60))
            .send()
            .await?;
        if let Err(error) = r.error_for_status_ref() {
            log::debug!(
                "Request status nok (status {}); retrieving message...",
                r.status()
            );
            let msg = r.text().await?;
            let msg = serde_json::from_str::<String>(&msg).unwrap_or(msg);
            return Err(Error::RelationGraph(error, msg));
        } else {
            log::debug!("Result sent to relation graph engine: {}", r.status());
        }
        Ok(())
    }
}
pub type Items = HashMap<String, ItemType>;
pub type Relations = HashMap<String, RelationType>;

#[derive(Debug, Serialize)]
#[serde(tag = "item_type")]
pub enum ItemType {
    #[serde(rename = "kubernetes/cluster")]
    Cluster(ClusterItem),
    #[serde(rename = "kubernetes/node")]
    Node(NodeItem),
    #[serde(rename = "kubernetes/namespace")]
    Namespace(NamespaceItem),
    #[serde(rename = "kubernetes/pod")]
    Pod(PodItem),
    #[serde(rename = "kubernetes/container")]
    Container(ContainerItem),
    #[serde(rename = "kubernetes/init_container")]
    InitContainer(InitContainerItem),
    #[serde(rename = "kubernetes/service")]
    Service(ServiceItem),
    #[serde(rename = "kubernetes/deployment")]
    Deployment(DeploymentItem),
    #[serde(rename = "kubernetes/stateful_set")]
    StatefulSet(StatefulSetItem),
    #[serde(rename = "kubernetes/storage_class")]
    StorageClass(StorageClassItem),
    #[serde(rename = "kubernetes/persistent_volume")]
    PersistentVolume(PersistentVolumeItem),
    #[serde(rename = "kubernetes/persistent_volume_claim")]
    PersistentVolumeClaim(PersistentVolumeClaimItem),
    #[serde(rename = "kubernetes/config_map")]
    ConfigMap(ConfigMapItem),
    #[serde(rename = "kubernetes/secret")]
    Secret(SecretItem),
    #[serde(rename = "kubernetes/daemon_set")]
    DaemonSet(DaemonSetItem),
    #[serde(rename = "kubernetes/cron_job")]
    CronJob(CronJobItem),
    #[serde(rename = "kubernetes/job")]
    Job(JobItem),
}

#[derive(Debug, Serialize)]
#[serde(tag = "relation_type")]
pub enum RelationType {
    #[serde(rename = "kubernetes/cluster-node")]
    ClusterNode(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-namespace")]
    ClusterNamespace(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-pod")]
    ClusterPod(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-service")]
    ClusterService(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-deployment")]
    ClusterDeployment(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-stateful_set")]
    ClusterStatefulSet(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-storage_class")]
    ClusterStorageClass(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-persistent_volume")]
    ClusterPersistentVolume(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-persistent_volume_claim")]
    ClusterPersistentVolumeClaim(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-config_map")]
    ClusterConfigMap(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-secret")]
    ClusterSecret(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-daemon_set")]
    ClusterDaemonSet(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-cron_job")]
    ClusterCronJob(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cluster-job")]
    ClusterJob(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/namespace-pod")]
    NamespacePod(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/namespace-service")]
    NamespaceService(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/namespace-deployment")]
    NamespaceDeployment(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/namespace-stateful_set")]
    NamespaceStatefulSet(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/namespace-persistent_volume_claim")]
    NamespacePersistentVolumeClaim(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/namespace-config_map")]
    NamespaceConfigMap(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/namespace-secret")]
    NamespaceSecret(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/namespace-daemon_set")]
    NamespaceDaemonSet(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/namespace-cron_job")]
    NamespaceCronJob(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/namespace-job")]
    NamespaceJob(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/node-pod")]
    NodePod(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/pod-container")]
    PodContainer(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/pod-init_container")]
    PodInitContainer(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/service-pod")]
    ServicePod(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/deployment-pod")]
    DeploymentPod(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/stateful_set-pod")]
    StatefulsetPod(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/pod-persistent_volume_claim")]
    PodPersistentVolumeClaim(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/pod-config_map")]
    PodConfigmap(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/pod-secret")]
    PodSecret(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/persistent_volume_claim-persistent_volume")]
    PersistentVolumeClaimPersistentVolume(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/storage_class-persistent_volume")]
    StorageClassPersistentVolume(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/daemon_set-pod")]
    DaemonSetPod(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/cron_job-pod")]
    CronJobPod(Relation<EmptyProperties>),
    #[serde(rename = "kubernetes/job-pod")]
    JobPod(Relation<EmptyProperties>),
}

#[derive(Debug, Serialize)]
pub struct StringProperty<T = String> {
    string: T,
}

#[derive(Debug, Serialize)]
pub struct TimeProperty<T = DateTime<Utc>> {
    time: T,
}

#[derive(Debug, Serialize)]
pub struct MapProperty<V, K = String> {
    map: BTreeMap<K, V>,
}

impl<T> StringProperty<T> {
    fn new(string: T) -> Self {
        Self { string }
    }
}

impl<T> TimeProperty<T> {
    fn new(time: T) -> Self {
        Self { time }
    }
}

impl<K, V> MapProperty<V, K> {
    fn new(map: BTreeMap<K, V>) -> Self {
        Self { map }
    }
}

#[derive(Debug, Serialize)]
pub struct ClusterItem {
    #[serde[with="prefix_kubernetes"]]
    properties: ClusterProperties,
}

#[derive(Debug, Serialize)]
struct ClusterProperties {
    name: StringProperty,
}

impl ClusterItem {
    pub fn new(name: String) -> Self {
        ClusterItem {
            properties: ClusterProperties {
                name: StringProperty::new(name),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NodeItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: NodeProperties,
}
#[derive(Debug, Serialize)]
pub struct NodeProperties {
    #[serde(flatten)]
    general_properties: ClusterResourceProperties,
}
impl NodeItem {
    pub fn new<U>(cluster: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        NodeItem {
            parent: cluster,
            properties: NodeProperties {
                general_properties: ClusterResourceProperties::new(resource),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NamespaceItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: NamespaceProperties,
}
#[derive(Debug, Serialize)]
pub struct NamespaceProperties {
    #[serde(flatten)]
    general_properties: ClusterResourceProperties,
}
impl NamespaceItem {
    pub fn new<U>(cluster: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        NamespaceItem {
            parent: cluster,
            properties: NamespaceProperties {
                general_properties: ClusterResourceProperties::new(resource),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PodItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: PodProperties,
}
#[derive(Debug, Serialize)]
pub struct PodProperties {
    #[serde(flatten)]
    general_properties: NamespaceResourceProperties,
}
impl PodItem {
    pub fn new<U>(namespace: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        PodItem {
            parent: namespace,
            properties: PodProperties {
                general_properties: NamespaceResourceProperties::new(resource),
            },
        }
    }
}
#[derive(Debug, Serialize)]
pub struct ContainerItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: ContainerProperties,
}
#[derive(Debug, Serialize)]
pub struct ContainerProperties {
    name: StringProperty,
    image: Option<StringProperty>,
}
impl ContainerItem {
    pub fn new(pod: String, name: String, image: Option<String>) -> Self {
        ContainerItem {
            parent: pod,
            properties: ContainerProperties {
                name: StringProperty::new(name),
                image: image.map(StringProperty::new),
            },
        }
    }
}
#[derive(Debug, Serialize)]
pub struct InitContainerItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: ContainerProperties,
}
impl InitContainerItem {
    pub fn new(pod: String, name: String, image: Option<String>) -> Self {
        InitContainerItem {
            parent: pod,
            properties: ContainerProperties {
                name: StringProperty::new(name),
                image: image.map(StringProperty::new),
            },
        }
    }
}
#[derive(Debug, Serialize)]
pub struct ServiceItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: ServiceProperties,
}
#[derive(Debug, Serialize)]
pub struct ServiceProperties {
    #[serde(flatten)]
    general_properties: NamespaceResourceProperties,
}
impl ServiceItem {
    pub fn new<U>(namespace: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        ServiceItem {
            parent: namespace,
            properties: ServiceProperties {
                general_properties: NamespaceResourceProperties::new(resource),
            },
        }
    }
}
#[derive(Debug, Serialize)]
pub struct DeploymentItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: DeploymentProperties,
}
#[derive(Debug, Serialize)]
pub struct DeploymentProperties {
    #[serde(flatten)]
    general_properties: NamespaceResourceProperties,
}
impl DeploymentItem {
    pub fn new<U>(namespace: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        DeploymentItem {
            parent: namespace,
            properties: DeploymentProperties {
                general_properties: NamespaceResourceProperties::new(resource),
            },
        }
    }
}
#[derive(Debug, Serialize)]
pub struct StatefulSetItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: StatefulSetProperties,
}
#[derive(Debug, Serialize)]
pub struct StatefulSetProperties {
    #[serde(flatten)]
    general_properties: NamespaceResourceProperties,
}
impl StatefulSetItem {
    pub fn new<U>(namespace: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        StatefulSetItem {
            parent: namespace,
            properties: StatefulSetProperties {
                general_properties: NamespaceResourceProperties::new(resource),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DaemonSetItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: DaemonSetProperties,
}
#[derive(Debug, Serialize)]
pub struct DaemonSetProperties {
    #[serde(flatten)]
    general_properties: NamespaceResourceProperties,
}
impl DaemonSetItem {
    pub fn new<U>(namespace: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        DaemonSetItem {
            parent: namespace,
            properties: DaemonSetProperties {
                general_properties: NamespaceResourceProperties::new(resource),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CronJobItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: CronJobProperties,
}
#[derive(Debug, Serialize)]
pub struct CronJobProperties {
    #[serde(flatten)]
    general_properties: NamespaceResourceProperties,
}
impl CronJobItem {
    pub fn new<U>(namespace: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        CronJobItem {
            parent: namespace,
            properties: CronJobProperties {
                general_properties: NamespaceResourceProperties::new(resource),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct JobItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: JobProperties,
}
#[derive(Debug, Serialize)]
pub struct JobProperties {
    #[serde(flatten)]
    general_properties: NamespaceResourceProperties,
}
impl JobItem {
    pub fn new<U>(namespace: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        JobItem {
            parent: namespace,
            properties: JobProperties {
                general_properties: NamespaceResourceProperties::new(resource),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StorageClassItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: StorageClassProperties,
}
#[derive(Debug, Serialize)]
pub struct StorageClassProperties {
    #[serde(flatten)]
    general_properties: ClusterResourceProperties,
}
impl StorageClassItem {
    pub fn new<U>(cluster: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        StorageClassItem {
            parent: cluster,
            properties: StorageClassProperties {
                general_properties: ClusterResourceProperties::new(resource),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PersistentVolumeItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: PersistentVolumeProperties,
}
#[derive(Debug, Serialize)]
pub struct PersistentVolumeProperties {
    #[serde(flatten)]
    general_properties: ClusterResourceProperties,
}
impl PersistentVolumeItem {
    pub fn new<U>(cluster: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        PersistentVolumeItem {
            parent: cluster,
            properties: PersistentVolumeProperties {
                general_properties: ClusterResourceProperties::new(resource),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PersistentVolumeClaimItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: PersistentVolumeClaimProperties,
}
#[derive(Debug, Serialize)]
pub struct PersistentVolumeClaimProperties {
    #[serde(flatten)]
    general_properties: NamespaceResourceProperties,
}
impl PersistentVolumeClaimItem {
    pub fn new<U>(namespace: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        PersistentVolumeClaimItem {
            parent: namespace,
            properties: PersistentVolumeClaimProperties {
                general_properties: NamespaceResourceProperties::new(resource),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConfigMapItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: ConfigMapItemProperties,
}
#[derive(Debug, Serialize)]
pub struct ConfigMapItemProperties {
    #[serde(flatten)]
    general_properties: NamespaceResourceProperties,
}
impl ConfigMapItem {
    pub fn new<U>(namespace: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        ConfigMapItem {
            parent: namespace,
            properties: ConfigMapItemProperties {
                general_properties: NamespaceResourceProperties::new(resource),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SecretItem {
    parent: String,
    #[serde[with="prefix_kubernetes"]]
    properties: SecretItemProperties,
}
#[derive(Debug, Serialize)]
pub struct SecretItemProperties {
    #[serde(flatten)]
    general_properties: NamespaceResourceProperties,
}
impl SecretItem {
    pub fn new<U>(namespace: String, resource: &U) -> Self
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        SecretItem {
            parent: namespace,
            properties: SecretItemProperties {
                general_properties: NamespaceResourceProperties::new(resource),
            },
        }
    }
}

pub trait RelationTrait {
    fn new<U: ResourceExt + Resource<DynamicType = ()>>(resource: &U) -> Self;
}
#[derive(Debug, Serialize)]
pub struct Relation<T: RelationTrait> {
    source: String,
    target: String,
    properties: T,
}
impl<T: RelationTrait> Relation<T> {
    pub fn new<U>(source: String, target: String, resource: &U) -> Relation<T>
    where
        U: ResourceExt + Resource<DynamicType = ()>,
    {
        Relation {
            source,
            target,
            properties: T::new(resource),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ClusterResourceProperties {
    apiversion: StringProperty,
    kind: StringProperty,
    name: StringProperty,
    uid: Option<StringProperty>,
    creation_timestamp: Option<TimeProperty<Time>>,
    labels: MapProperty<StringProperty>,
}

impl ClusterResourceProperties {
    fn new<T>(resource: &T) -> ClusterResourceProperties
    where
        T: ResourceExt + Resource<DynamicType = ()>,
    {
        ClusterResourceProperties {
            apiversion: StringProperty::new(T::api_version(&()).to_string()),
            kind: StringProperty::new(T::kind(&()).to_string()),
            name: StringProperty::new(resource.name_any()),
            uid: resource.uid().map(StringProperty::new),
            creation_timestamp: resource.creation_timestamp().map(TimeProperty::new),
            labels: MapProperty::new(
                resource
                    .labels()
                    .iter()
                    .map(|(k, v)| (k.clone(), StringProperty::new(v.clone())))
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NamespaceResourceProperties {
    namespace: Option<StringProperty>,
    #[serde(flatten)]
    cluster_properties: ClusterResourceProperties,
}
impl NamespaceResourceProperties {
    fn new<T>(resource: &T) -> NamespaceResourceProperties
    where
        T: ResourceExt + Resource<DynamicType = ()>,
    {
        NamespaceResourceProperties {
            namespace: resource.namespace().map(StringProperty::new),
            cluster_properties: ClusterResourceProperties::new(resource),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EmptyProperties {}
impl RelationTrait for EmptyProperties {
    fn new<U: ResourceExt + Resource<DynamicType = ()>>(_resource: &U) -> Self {
        EmptyProperties {}
    }
}
