# Static hostPath volume (optional)

`k8s/setting/githubissuemanager-persistentvolumeclaim.yaml` uses the cluster's **default
StorageClass**, which works out of the box on minikube, kind and managed Kubernetes.

Use these manifests only on a single-node cluster that has **no** dynamic provisioner:

1. Apply the StorageClass and PersistentVolume:

       kubectl apply -f k8s-extras/static-hostpath-volume/

2. Add `storageClassName: local-storage` to the claim's `spec` in
   `k8s/setting/githubissuemanager-persistentvolumeclaim.yaml`, then apply `k8s/` as usual.

The volume uses `persistentVolumeReclaimPolicy: Retain`, so deleting the claim keeps the
database files on the node. hostPath volumes are tied to one node and are not suitable for
multi-node production clusters.
