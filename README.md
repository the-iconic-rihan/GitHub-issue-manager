
# Table of Contents

1.  [Getting Started](#orge319bec)
    1.  [Create Docker images](#orgd624d66)
    2.  [Server](#orgfa88653)
    3.  [Website](#org8bf92b4)
    4.  [Enable `ingress` addon for `minikube`](#org4571b11)
    5.  [Setup](#orgc782791)
        1.  [`gi-cli`](#org513ecb8)
        2.  [Manual Setup](#orgaf135a2)
2.  [Data persistence across restarts](#org27ce41a)
3.  [Reuse Docker daemon from Minikube](#org36108f6)
4.  [Development](#org0d391eb)
    1.  [Minikube tunnel](#org7d551c7)
    2.  [Tunnel Setup](#orgf2eb6f8)



<a id="orge319bec"></a>

# Getting Started

Make sure `libvirtd` service is up and running

    sudo rc-service libvirtd start

Start `minikube`

    minikube start


<a id="orgd624d66"></a>

## Create Docker images

[Use `minikube` docker daemon](#org36108f6)

    eval $(minikube -p minikube docker-env)


<a id="orgfa88653"></a>

## Server

From project root run

    docker build -t githubissuemanager/website website/


<a id="org8bf92b4"></a>

## Website

From project root run

    docker build -t githubissuemanager/server app/


<a id="org4571b11"></a>

## Enable `ingress` addon for `minikube`

    minikube addons enable ingress


<a id="orgc782791"></a>

## Setup


<a id="org513ecb8"></a>

### `gi-cli`

Use `gi-cli` to bootstrap clusters

    gi-cli cluster bootstrap -a apply -p <path-to-k8s>


<a id="orgaf135a2"></a>

### Manual Setup

1.  Create `githubissuemanager` namespace

        kubectl create namespace githubissuemanager

2.  Initialize cluster

        kubectl apply -R -f k8s/
        networkpolicy.networking.k8s.io/client-side created
        ingress.networking.k8s.io/githubissuemanager-ngnix-ingress created
        persistentvolume/githubissuemanager-pv configured
        persistentvolumeclaim/githubissuemanager-claim created
        secret/githubissuemanager-secret created
        configmap/init-database created
        service/postgresdb created
        statefulset.apps/postgresdb created
        deployment.apps/server created
        service/server created
        networkpolicy.networking.k8s.io/server-side created
        deployment.apps/website created
        service/website created

3.  Browse

    Check `minikube service list` to see what ports are being used for deployment
    
        minikube service list
        |--------------------+------------------------------------+--------------+-----------------------------|
        | NAMESPACE          | NAME                               | TARGET PORT  | URL                         |
        |--------------------+------------------------------------+--------------+-----------------------------|
        | default            | kubernetes                         | No node port |                             |
        | githubissuemanager | postgresdb                         | No node port |                             |
        | githubissuemanager | server                             | No node port |                             |
        | githubissuemanager | website                            | No node port |                             |
        | ingress-nginx      | ingress-nginx-controller           | http/80      | http://192.168.39.196:30297 |
        |                    |                                    | https/443    | http://192.168.39.196:31388 |
        | ingress-nginx      | ingress-nginx-controller-admission | No node port |                             |
        | kube-system        | kube-dns                           | No node port |                             |
        |--------------------+------------------------------------+--------------+-----------------------------|
    
    From this example, open `http://192.168.39.196:30297` in browser.


<a id="org27ce41a"></a>

# Data persistence across restarts

From [Minikube Handbook](https://minikube.sigs.k8s.io/docs/handbook/persistent_volumes/)

> A note on mounts, persistence, and minikube hosts
> 
> minikube is configured to persist files stored under the following directories, which are made in the Minikube VM (or on your localhost if running on bare metal). You may lose data from other directories on reboots.
> 
> /data\*
> /var/lib/minikube
> /var/lib/docker
> /var/lib/containerd
> /var/lib/buildkit
> /var/lib/containers
> /tmp/hostpath<sub>pv</sub>\*
> /tmp/hostpath-provisioner\*
> 
> \*mount point for another directory, stored under /var or on a separate data disk

So all data will persist as long as `githubissuemanager-persistentvolume` isn&rsquo;t deleted.


<a id="org36108f6"></a>

# Reuse Docker daemon from Minikube

Instead of Kubernetes pulling images as described by the `image` tag

    image: githubissuemanager/server
    name: server

Kubernetes can use locally built image

1.  Set the environment variables with `eval $(minikube docker-env)`
2.  Build image with Minikube&rsquo;s Docker daemon `docker build -t my-image .`
3.  Set the image in the pod spec
4.  Set the `imagePullPolicy` to `Never`

    image: githubissuemanager/server
    imagePullPolicy: Never
    name: server


<a id="org0d391eb"></a>

# Development


<a id="org7d551c7"></a>

## Minikube tunnel

Using a custom Kubernetes Cluster (using minikube, kubeadm, etc) means there&rsquo;s no `LoadBalancer` integration (unlike AWS or Google Cloud).
With this default setup, there&rsquo;s only `NodePort` or an `Ingress Controller`.

But a `LoadBalancer` can be *emulated* by Minikube using `minikube tunnel` command.

See [k8s handbook, uisng minikube](https://minikube.sigs.k8s.io/docs/handbook/accessing/#using-minikube-tunnel)


<a id="orgf2eb6f8"></a>

## Tunnel Setup

Depending on minikube driver (usually docker), minikube can have different port forwarding. `kvm2` driver constructs images on host, which means no need to tunnel (different from minikube cmd `tunnel`).

For native host

1.  Delete existing image
    
        minikube delete

2.  Change driver
    
        minikube config set vm-driver kvm2

3.  Start again
    
        minikube start

Now listing exposed services will be correctly mapped.

minikube service list

    |--------------------+------------+--------------+-----------------------------|
    | NAMESPACE          | NAME       | TARGET PORT  | URL                         |
    |--------------------+------------+--------------+-----------------------------|
    | default            | kubernetes | No node port |                             |
    | githubissuemanager | postgresdb | 5432/5432    | http://192.168.39.232:31524 |
    | githubissuemanager | server     | 8080/8080    | http://192.168.39.232:31933 |
    | githubissuemanager | website    | 3000/3000    | http://192.168.39.232:32231 |
    | kube-system        | kube-dns   | No node port |                             |
    |--------------------+------------+--------------+-----------------------------|


#made something