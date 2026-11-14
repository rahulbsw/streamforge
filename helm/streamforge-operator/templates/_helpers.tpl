{{/*
Expand the name of the chart.
*/}}
{{- define "streamforge-operator.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Resolve the Secret containing the UI JWT signing key.
*/}}
{{- define "streamforge-operator.uiSecretName" -}}
{{- if .Values.ui.auth.existingSecret -}}
{{- .Values.ui.auth.existingSecret -}}
{{- else -}}
{{- printf "%s-ui-secret" (include "streamforge-operator.fullname" .) -}}
{{- end -}}
{{- end }}

{{/*
Use the explicitly configured image tag, otherwise the application version.
*/}}
{{- define "streamforge-operator.imageTag" -}}
{{- default (index . 1) (index . 0) -}}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "streamforge-operator.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "streamforge-operator.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "streamforge-operator.labels" -}}
helm.sh/chart: {{ include "streamforge-operator.chart" . }}
{{ include "streamforge-operator.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "streamforge-operator.selectorLabels" -}}
app.kubernetes.io/name: {{ include "streamforge-operator.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "streamforge-operator.serviceAccountName" -}}
{{- if .Values.operator.serviceAccount.create }}
{{- default (include "streamforge-operator.fullname" .) .Values.operator.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.operator.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Pipeline service account name
*/}}
{{- define "streamforge-operator.pipelineServiceAccountName" -}}
{{- if .Values.defaults.serviceAccount.create }}
{{- default "streamforge-pipeline" .Values.defaults.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.defaults.serviceAccount.name }}
{{- end }}
{{- end }}
