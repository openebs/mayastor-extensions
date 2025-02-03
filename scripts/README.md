# Enabling TLS

1. Push images from [rest_tls][rest_tls] branch on control plane.
    - ```./scripts/release.sh --registry <registry url> --alias-tag <tag> --image rest operators.diskpool csi.controller```
1. Enable TLS in values.yaml [here][enableTls].
1. With cluster in current context, run certs.sh from scripts dir. This will create certs and ultimately a kubernetes secret containing those certs.
1. install mayastor.
    - ensure rest, diskpool operator and csi controller are using images from previous step.

[enableTls]: https://github.com/Johnaius/mayastor-extensions/blob/tls/chart/values.yaml#L94-L95
[rest_tls]: https://github.com/Johnaius/mayastor-control-plane/tree/rest_tls