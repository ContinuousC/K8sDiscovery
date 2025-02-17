[![Build Status](https://drone.contc/api/badges/ContinuousC/K8sDiscovery/status.svg)](https://drone.contc/ContinuousC/K8sDiscovery)

This Helm Chart installs the ContinuousC Kubernetes discovery and metrics shipper.

# Installation

* Create the continuousc namespace on the cluster you want to monitor.

  ```
  kubectl --context $CONTEXT create namespace continuousc
  ```

* Create a file `secret.yaml` containing the remote write secret. Do
  not commit this file to any repository.

  ```
  apiVersion: v1
  kind: Secret
  metadata:
    name: credentials
  stringData:
    discovery.secret: $DISCOVERY_SECRET
    prometheus_remote_write.secret: $REMOTE_WRITE_SECRET
  ```

* Load the secret in kubernetes:

  ```
  kubelet --context $CONTEXT create -n continuousc -f secret.yaml && rm -f secret.yaml
  ```

* Make sure the ContinuousC Helm repo is added to your Helm installation.

  ```
  helm repo list | grep '^continuousc ' || helm repo add continuousc https://gitea.contc/api/packages/ContinuousC/helm
  helm repo update
  ```

* Create a file `config.yaml` to hold your cluster-specific
  configuration. You can use another filename, provided you change it
  in the command below. This file can be versioned in a git repository
  if desired. Use your own secret distribution mechanism as required.
  
  Replace all the variables with suitable values.

  ```
  discovery:
    cluster: $UNIQUE_CLUSTER_NAME
    engineUrl: https://$TENANT_DOMAIN/api/
	token:
	  url: https://sso.continuousc.contc/realms/$REALM/protocol/openid-connect/token
	  userName: $DISCOVERY_USER
  prometheus:
    server:
      global:
        external_labels:
          cluster: $UNIQUE_CLUSTER_NAME
      remoteWrite:
        - url: https://$TENANT_DOMAIN/api/v1/push
          oauth2:
            token_url: https://sso.continuousc.contc/realms/$REALM/protocol/openid-connect/token
            client_id: $REMOTE_WRITE_USER
            client_secret_file: /etc/secrets/prometheus_remote_write.secret
            tls_config:
              ca_file: /etc/continuousc/ca.crt
          tls_config:
            ca_file: /etc/continuousc/ca.crt
      extraSecretMounts:
        - name: credentials
          secretName: credentials
          mountPath: /etc/secrets
  ```

* Finally, install the helm chart to the cluster you want to monitor:

  ```
  helm install --kube-context $CONTEXT --namespace continuousc --values config.yaml --create-namespace k8s-discovery continuousc/k8s-discovery
  ```

# Update

* To update the helm chart, run:

  ```
  helm repo update
  helm upgrade  --kube-context $CONTEXT --namespace continuousc --values config.yaml k8s-discovery continuousc/k8s-discovery
  ```
