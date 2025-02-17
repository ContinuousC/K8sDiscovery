# Development

## Install dependencies:

### devspace

```
npm install -g devspace

```

### argocd-vault-plugin

```
sudo curl -L https://github.com/argoproj-labs/argocd-vault-plugin/releases/download/v1.16.1/argocd-vault-plugin_1.16.1_linux_amd64 \
  -o argocd-vault-plugin && chmod +x argocd-vault-plugin && mv argocd-vault-plugin /usr/bin
```

## Configuration

### kubernetes

Access to kubernetes cluster by config in ~/.kube/config

### devspace

```
devspace use context                  # to select the right Kubernetes cluster
devspace use namespace $NAMESPACE     # namespace is unique working namespace in k8s cluster for a developer
```

### vault

TODO:
should be done by a tool wich use a configuration file

For the moment:
Get a token for vault with access to your specific secrets in vault
~/.vaultrc.yaml with content:
AVP_TYPE: vault
AVP_AUTH_TYPE: token
VAULT_ADDR: https://vault.contc
VAULT_TOKEN: $TOKEN

## Developing

Whenever the Chart is updated

```
devspace deploy
```

This will run a dev container and auto-sync chanegs in source code and resync them

```
devspace dev
```
