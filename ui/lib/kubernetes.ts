import * as k8s from '@kubernetes/client-node';

const kubeConfig = new k8s.KubeConfig();
kubeConfig.loadFromDefault();

export const customObjectsApi = kubeConfig.makeApiClient(k8s.CustomObjectsApi);
export const coreApi = kubeConfig.makeApiClient(k8s.CoreV1Api);
export const appsApi = kubeConfig.makeApiClient(k8s.AppsV1Api);

export const PIPELINE_GROUP = 'streamforge.io';
export const PIPELINE_VERSION = 'v1alpha1';
export const PIPELINE_PLURAL = 'streamforgepipelines';
