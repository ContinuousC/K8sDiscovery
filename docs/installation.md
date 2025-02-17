On the cluster where k8s-discovery should run

- on first install

```
helm install k8s-discovery continuousc/k8s-discovery --set app.imageRegistryBase=gitea.contc/continuousc --set discovery.cluster=cluster-demo --set discovery.intervalMinutes=1 --set discovery.engineUrl=http://$TENAN_URL/api/ --namespace tenant-demo --insecure-skip-tls-verify

```

- on update

```
help repo update
helm upgrade k8s-discovery continuousc/k8s-discovery --set app.imageRegistryBase=gitea.contc/continuousc --set discovery.cluster=cluster-demo --set discovery.intervalMinutes=1 --set discovery.engineUrl=http://$TENAN_URL/api/ --namespace tenant-demo --insecure-skip-tls-verify

```
