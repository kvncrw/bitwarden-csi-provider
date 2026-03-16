# bitwarden-csi-provider

Bitwarden Secrets Manager provider for the [Kubernetes Secrets Store CSI Driver](https://secrets-store-csi-driver.sigs.k8s.io/).

Mounts secrets from Bitwarden Secrets Manager directly as tmpfs files in pods — secrets never persist to etcd as Kubernetes Secret objects.

## How it works

The Secrets Store CSI Driver discovers providers via Unix sockets in `/etc/kubernetes/secrets-store-csi-providers/`. This provider implements the CSI Driver's gRPC contract (Version + Mount RPCs), fetching secrets from Bitwarden Secrets Manager on each pod mount.

## Usage

### 1. Install the Secrets Store CSI Driver

```bash
helm repo add secrets-store-csi-driver https://kubernetes-sigs.github.io/secrets-store-csi-driver/charts
helm install csi-secrets-store secrets-store-csi-driver/secrets-store-csi-driver \
  --namespace kube-system
```

### 2. Deploy the provider

```bash
helm install bitwarden-csi-provider deploy/helm/bitwarden-csi-provider/ \
  --namespace kube-system
```

### 3. Create a Secret with your BSM access token

```bash
kubectl create secret generic bws-token \
  --from-literal=access_token="0.your-access-token-here"
```

### 4. Create a SecretProviderClass

```yaml
apiVersion: secrets-store.csi.x-k8s.io/v1
kind: SecretProviderClass
metadata:
  name: my-secrets
spec:
  provider: bitwarden
  secretObjects: []
  parameters:
    objects: |
      - id: "uuid-of-secret"
        path: "my-secret"
      - project: "uuid-of-project"
        pathPrefix: "project/"
```

### 5. Mount in a pod

```yaml
volumes:
  - name: secrets
    csi:
      driver: secrets-store.csi.k8s.io
      readOnly: true
      volumeAttributes:
        secretProviderClass: my-secrets
      nodePublishSecretRef:
        name: bws-token
containers:
  - volumeMounts:
      - name: secrets
        mountPath: /mnt/secrets
        readOnly: true
```

## Building

```bash
cargo build --release
docker build -t ghcr.io/kvncrw/bitwarden-csi-provider:latest .
```

## Testing

```bash
# Unit tests
cargo test --workspace

# Integration tests (requires fake-server)
BWS_FAKE_SERVER_BIN=bws-fake-server cargo test --workspace
```

## License

GPL-3.0-only
