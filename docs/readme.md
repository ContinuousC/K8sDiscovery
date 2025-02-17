[![Build Status](https://drone.contc/api/badges/ContinuousC/K8sDiscovery/status.svg)](https://drone.contc/ContinuousC/K8sDiscovery)

Should actually be called K8sAgent and work in conjuction with C9C:

- The K8sDiscovery component helps discover items and relation of kubernetes objects such as pods, deployments, etc...
- The Prometheus exporter will export metrics to our cortex instance

TODO:

- Make sure tenant access is securly setup for the prometheus exporter. For the momement you just can set any header (tenant-name) and directly get access to cortex. Solution is to intercept this via traefik proxy middleware as cortext cannot be setup with SSO.
