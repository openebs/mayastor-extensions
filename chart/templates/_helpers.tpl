{{/* vim: set filetype=mustache: */}}

{{/*
Renders a value that contains template.
Usage:
{{ include "render" ( dict "value" .Values.path.to.the.Value "context" $) }}
*/}}
{{- define "render" -}}
    {{- if typeIs "string" .value }}
        {{- tpl .value .context }}
    {{- else }}
        {{- tpl (.value | toYaml) .context }}
    {{- end }}
{{- end -}}

{{/*
Renders the CORE server init container, if enabled
Usage:
{{ include "base_init_core_containers" . }}
*/}}
{{- define "base_init_core_containers" -}}
    {{- if .Values.base.initCoreContainers.enabled }}
    {{- include "render_init_containers" (dict "value" .Values.base.initCoreContainers.containers "context" $) | nindent 8 }}
    {{- end }}
{{- end -}}

{{/*
Renders the HA NODE AGENT init container, if enabled
Usage:
{{ include "base_init_ha_node_containers" . }}
*/}}
{{- define "base_init_ha_node_containers" -}}
    {{- if .Values.base.initHaNodeContainers.enabled }}
    {{- include "render_init_containers" (dict "value" .Values.base.initHaNodeContainers.containers "context" $) | nindent 8 }}
    {{- end }}
{{- end -}}

{{/*
Renders the base init containers for all deployments, if any
Usage:
{{ include "base_init_containers" . }}
*/}}
{{- define "base_init_containers" -}}
    {{- if .Values.base.initContainers.enabled }}
    {{- include "render_init_containers" (dict "value" .Values.base.initContainers.containers "context" $) | nindent 8 }}
    {{- end }}
    {{- include "jaeger_collector_init_container" . }}
{{- end -}}

{{/*
Renders the jaeger agent init container, if enabled
Usage:
{{ include "jaeger_collector_init_container" . }}
*/}}
{{- define "jaeger_collector_init_container" -}}
    {{- if .Values.base.jaeger.enabled }}
      {{- if .Values.base.jaeger.initContainer }}
      {{- if .Values.base.jaeger.collector }}
      {{- include "render_init_containers" (dict "value" .Values.base.jaeger.collector.initContainer "context" $) | nindent 8 }}
      {{- else }}
        - name: jaeger-probe
          image: busybox:latest
          command: [ 'sh', '-c', 'trap "exit 1" TERM; until nc -vzw 5 -u jaeger-collector:4317; do date; echo "Waiting for jaeger..."; sleep 1; done;' ]
      {{- end }}
      {{- end }}
    {{- end }}
{{- end -}}

{{/*
Renders the csi node init containers, if enabled
Usage:
{{ include "csi_node_init_containers" . }}
*/}}
{{- define "csi_node_init_containers" -}}
    {{- if (.Values.csi.node.initContainers).enabled }}
    {{- include "render_init_containers" (dict "value" .Values.csi.node.initContainers.containers "context" $) | nindent 8 }}
    {{- end }}
{{- end -}}

{{/*
Renders the base image pull secrets for all deployments, if any
Usage:
{{ include "base_pull_secrets" . }}
*/}}
{{- define "base_pull_secrets" -}}
    {{- if (not (empty .Values.global.imagePullSecrets)) }}
        {{- range .Values.global.imagePullSecrets | uniq -}}
            {{- if kindIs "map" . -}}
            {{ nindent 8 "- name:" }} {{ .name }}
            {{- else -}}
            {{ nindent 8 "- name:" }} {{ . }}
            {{- end }}
        {{- end }}
    {{- else if (not (empty .Values.image.pullSecrets)) }}
        {{- range .Values.image.pullSecrets | uniq -}}
            {{- if kindIs "map" . -}}
            {{ nindent 8 "- name:" }} {{ .name }}
            {{- else -}}
            {{ nindent 8 "- name:" }} {{ . }}
            {{- end }}
        {{- end }}
    {{- else -}}
        {{- if .Values.base.imagePullSecrets }}
            {{- if .Values.base.imagePullSecrets.enabled }}
                {{- if (empty .Values.base.imagePullSecrets.secrets) }}
                    {{ nindent 8 "- name: login" }}
                {{- else -}}
                    {{- include "render" (dict "value" .Values.base.imagePullSecrets.secrets "context" $) | nindent 8 }}
                {{- end}}
            {{- end }}
        {{- end }}
    {{- end }}
{{- end -}}

{{/*
Concatenates imagepullsecrets for preupgradehook and handles different formats (example - secret or - name: secret)
*/}}
{{- define "mayastor.preUpgradeHook.pullSecrets" -}}
{{- $names := list -}}
{{- with .Values.global.imagePullSecrets -}}
  {{- range . -}}
    {{- if kindIs "map" . }}
      {{- if and (hasKey . "name") (not (empty .name)) -}}
        {{ $names = append $names .name }}
      {{- end -}}
    {{- else if not (empty .) -}}
      {{ $names = append $names . -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- with .Values.preUpgradeHook.imagePullSecrets -}}
  {{- range . }}
    {{- if kindIs "map" . -}}
      {{- if and (hasKey . "name") (not (empty .name)) -}}
        {{- $names = append $names .name }}
      {{- end -}}
    {{- else if not (empty .) -}}
      {{- $names = append $names . -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- $names = uniq $names -}}
{{- if $names -}}
  {{- range $names }}
- name: {{ . }}
  {{- end -}}
{{- end -}}
{{- end -}}

{{/*
Renders the REST server init container, if enabled
Usage:
{{- include "rest_agent_init_container" . }}
*/}}
{{- define "rest_agent_init_container" -}}
    {{- if .Values.base.initRestContainer.enabled }}
    {{- include "render_init_containers" (dict "value" .Values.base.initRestContainer.initContainer "context" $) | nindent 8 }}
    {{- end }}
{{- end -}}

{{/*
Renders the jaeger scheduling rules, if any
Usage:
{{ include "jaeger_scheduling" . }}
*/}}
{{- define "jaeger_scheduling" -}}
    {{- if index .Values "jaeger-operator" "affinity" }}
  affinity:
    {{- include "render" (dict "value" (index .Values "jaeger-operator" "affinity") "context" $) | nindent 4 }}
    {{- end }}
    {{- if index .Values "jaeger-operator" "tolerations" }}
  tolerations:
    {{- include "render" (dict "value" (index .Values "jaeger-operator" "tolerations") "context" $) | nindent 4 }}
    {{- end }}
{{- end -}}

{{/* Generate Core list specification (-l param of io-engine) */}}
{{- define "cpuFlag" -}}
{{- include "coreListUniq" . -}}
{{- end -}}

{{/* Get the number of cores from the coreList */}}
{{- define "coreCount" -}}
{{- include "coreListUniq" . | split "," | len -}}
{{- end -}}

{{- define "logFormat" -}}
{{- if (regexMatch "^((json|pretty|compact))$" .Values.base.logging.format) -}}
    {{- print .Values.base.logging.format -}}
{{- else -}}
    {{- fail "invalid logging format. valid values are json, pretty, compact" -}}
{{- end -}}
{{- end -}}

{{/* Get a list of cores as a comma-separated list */}}
{{- define "coreListUniq" -}}
{{- if .Values.io_engine.coreList -}}
{{- $cores_pre := .Values.io_engine.coreList -}}
{{- if not (kindIs "slice" .Values.io_engine.coreList) -}}
{{- $cores_pre = list $cores_pre -}}
{{- end -}}
{{- $cores := list -}}
{{- range $index, $value := $cores_pre | uniq -}}
{{- $value = $value | toString | replace " " "" }}
{{- if eq ($value | int | toString) $value -}}
{{-   $cores = append $cores $value -}}
{{- end -}}
{{- end -}}
{{- $first := first $cores | required (print "At least one core must be specified in io_engine.coreList") -}}
{{- $cores | join "," -}}
{{- else -}}
{{- if gt 1 (.Values.io_engine.cpuCount | int) -}}
{{- fail ".Values.io_engine.cpuCount must be >= 1" -}}
{{- end -}}
{{- untilStep 1 (add 1 .Values.io_engine.cpuCount | int) 1 | join "," -}}
{{- end -}}
{{- end }}

{{/*
Adds the project domain to labels
Usage:
{{ include "label_prefix" . }}/release: {{ .Release.Name }}
*/}}
{{- define "label_prefix" -}}
    {{ $product := .Files.Get "product.yaml" | fromYaml }}
    {{- print $product.domain -}}
{{- end -}}

{{/*
Creates the tolerations based on the global and component wise tolerations, with early eviction
Usage:
{{ include "_tolerations_with_early_eviction" (dict "template" . "localTolerations" .Values.path.to.local.tolerations) }}
*/}}
{{- define "_tolerations_with_early_eviction" -}}
{{- toYaml .template.Values.earlyEvictionTolerations | nindent 8 }}
{{- if .localTolerations }}
    {{- toYaml .localTolerations | nindent 8 }}
{{- else if .template.Values.tolerations }}
    {{- toYaml .template.Values.tolerations | nindent 8 }}
{{- end }}
{{- end }}


{{/*
Creates the tolerations based on the global and component wise tolerations
Usage:
{{ include "tolerations" (dict "template" . "localTolerations" .Values.path.to.local.tolerations) }}
*/}}
{{- define "tolerations" -}}
{{- if .localTolerations }}
    {{- toYaml .localTolerations | nindent 8 }}
{{- else if .template.Values.tolerations }}
    {{- toYaml .template.Values.tolerations | nindent 8 }}
{{- end }}
{{- end }}

{{/*
Creates the node selector based on the global and component wise node selectors
Usage:
{{ include "node_selector" (dict "template" . "localNodeSelector" .Values.path.to.local.nodeSelector) }}
*/}}
{{- define "node_selector" -}}
{{- if .localNodeSelector }}
    {{- toYaml .localNodeSelector | nindent 8 }}
{{- else if .template.Values.nodeSelector }}
    {{- toYaml .template.Values.nodeSelector | nindent 8 }}
{{- end }}
{{- end }}

{{/*
Generates the priority class name, with the given `template` and the `localPriorityClass`
Usage:
{{ include "priority_class" (dict "template" . "localPriorityClass" .Values.path.to.local.priorityClassName) }}
*/}}
{{- define "priority_class" -}}
    {{- if typeIs "string" .localPriorityClass }}
        {{- if .localPriorityClass -}}
            {{ printf "%s" .localPriorityClass -}}
        {{- else if .template.Values.priorityClassName -}}
            {{ printf "%s" .template.Values.priorityClassName -}}
        {{- else -}}
            {{ printf "" -}}
        {{- end -}}
    {{- end -}}
{{- end -}}


{{/*
Generates the priority class name, with the given `template` and the `localPriorityClass`, sets to mayastor default priority class
if both are empty
Usage:
{{ include "priority_class_with_default" (dict "template" . "localPriorityClass" .Values.path.to.local.priorityClassName) }}
*/}}
{{- define "priority_class_with_default" -}}
    {{- if typeIs "string" .localPriorityClass }}
        {{- if .localPriorityClass -}}
            {{ printf "%s" .localPriorityClass -}}
        {{- else if .template.Values.priorityClassName -}}
            {{ printf "%s" .template.Values.priorityClassName -}}
        {{- else -}}
            {{ printf "%s-cluster-critical" .template.Release.Name -}}
        {{- end -}}
    {{- end -}}
{{- end -}}

{{/*
    Generate the default StorageClass parameters.
    This is required because StorageClass parameters cannot be patched after creation.
    If the StorageClass already exists, the default StorageClass carries the parameters and values
    of that StorageClass. Else, it carries the default parameters and values.
*/}}
{{- define "storageClass.parameters" -}}
    {{- $scName := index . 0 -}}
    {{- $valuesParams := index . 1 -}}

    {{/* Check to see if a default StorageClass already exists */}}
    {{- $sc := lookup "storage.k8s.io/v1" "StorageClass" "" $scName -}}

    {{- if $sc -}}
        {{/* Existing defaults */}}
        {{ range $param, $val := $sc.parameters }}
{{ $param | quote }}: {{ $val | quote }}
        {{- end -}}

    {{- else -}}
        {{/* Current defaults */}}
        {{ range $param, $val := $valuesParams }}
{{ $param | quote }}: {{ $val | quote }}
        {{- end -}}
    {{- end -}}
{{- end -}}

{{/*
Adds the image prefix to image name
*/}}
{{- define "image_prefix" -}}
    {{ $product := .Files.Get "product.yaml" | fromYaml }}
    {{- print $product.imagePrefix -}}
{{- end -}}

{{/*
Get the Jaeger URL
*/}}
{{- define "jaeger_url" -}}
    {{- if $collector := .Values.base.jaeger.collector }}
        {{- $collector.name }}:{{ $collector.port }}
    {{- else }}
        {{- print "jaeger-collector:4317" -}}
    {{- end }}
{{- end -}}

{{/*
 Create a normalized etcd name based on input parameters
 */}}
{{- define "etcdUrl" -}}
    {{- if eq (.Values.etcd.enabled) false }}
        {{- if .Values.etcd.externalUrl }}
            {{- .Values.etcd.externalUrl }}
        {{- else }}
          {{- fail "etcd.externalUrl must be set" }}
        {{- end }}
    {{- else }}
        {{- .Release.Name }}-etcd:{{ .Values.etcd.service.ports.client }}
    {{- end }}
{{- end }}

{{/*
 Check if etcd is explicitly enabled/disabled or implicitly enabled (for upgrades where enabled key was absent)
 */}}
{{- define "etcdEnabled" -}}
    {{- if eq (.Values.etcd.enabled) false }}
        {{- "false" -}}
    {{- else if eq (.Values.etcd.enabled) true }}
        {{- "true" -}}
    {{- else if .Values.etcd.externalUrl }}
        {{- "false" -}}
    {{- else }}
        {{- "true" -}}
    {{- end }}
{{- end }}

{{/*
Renders init containers. If unset it sets the container image.
*/}}
{{- define "render_init_containers" -}}
    {{- $containers := list }}
    {{- $image := .context.Values.base.initContainers.image }}
    {{- $values_image := .context.Values.image }}
    {{- $global := .context.Values.global }}
    {{- $ctx := .context }}
    {{- range .value -}}
        {{ $container := deepCopy . }}
        {{- if hasKey $container "command" }}
          {{- $renderedCmd := list }}
            {{- range $container.command }}
              {{- $renderedCmd = append $renderedCmd (tpl . $ctx) }}
            {{- end }}
           {{- $_ := set $container "command" $renderedCmd }}
        {{- end }}
        {{- if not (hasKey $container "imagePullPolicy") }}
            {{- $pullPolicy := $global.imagePullPolicy | default $image.pullPolicy | default $values_image.pullPolicy }}
            {{- $_ := set $container "imagePullPolicy" $pullPolicy }}
        {{- end }}
        {{- if not (hasKey $container "image") }}
            {{- $_ := set $container "image" (include "render_init_container_image" $ctx ) }}
        {{- end }}
        {{- $containers = append $containers $container }}
    {{- end -}}
    {{- $containers | toYaml }}
{{- end -}}

{{- define "render_init_container_image" -}}
{{- $image := .Values.base.initContainers.image }}
{{- $values_image := .Values.image }}
{{- $global := .Values.global }}
{{- $registry := $global.imageRegistry | default $image.registry | default $values_image.registry }}
{{- $namespace := $image.namespace | default $values_image.repo }}
{{- $name := $image.name | default "alpine-sh" }}
{{- $tag := $image.tag | default "4.1.0" }}
{{- printf "%s/%s/%s:%s" $registry $namespace $name $tag }}
{{- end -}}


{{/*
Get the Events Jetstream Replica Count
*/}}
{{- define "events_replicas" -}}
    {{- if .Values.nats.cluster.enabled }}
        {{- min .Values.nats.cluster.replicas 3 }}
    {{- else }}
        {{- print "1" -}}
    {{- end }}
{{- end -}}

{{/*
Returns matched if the Etcd StatefulSet is of v8.6.0
Usage:
  {{- if include "etcd_is_8.6.0" . }}
    Do something
  {{- end }}
*/}}
{{- define "etcd_is_8.6.0" -}}
  {{- $sts  := lookup "apps/v1" "StatefulSet" .Release.Namespace (printf "%s-etcd" .Release.Name) -}}
  {{/*
  If no STS exists, erring on the side of caution and assuming there is one
  and we made a mistake in finding it --> matched
  */}}
  {{- if not $sts -}}
    matched
  {{- else -}}
    {{- if and $sts.metadata $sts.metadata.labels -}}
      {{/* Grab value of chart label (or default to "") */}}
      {{- $chart_name := index $sts.metadata.labels "helm.sh/chart" | default "" -}}
      {{/* If it’s exactly "etcd-8.6.0" --> matched */}}
      {{- if eq $chart_name "etcd-8.6.0" -}}
        matched
      {{- end -}}
    {{- else -}}
      {{/*
      $sts exists, but doesn't have .metadata or .metadata.labels for some reason.
      This may happen in a dry-run due to how lookup behaves. Erring on the side of caution and matching.
      */}}
      matched
    {{- end -}}
  {{- end -}}
{{- end }}

{{/*
Validates tls settings and fails with a clear message on invalid combinations.
Usage: {{ include "validate_tls_mode" . }}
*/}}
{{- define "validate_tls_mode" -}}
{{- $globalTls := .Values.security.tls -}}
{{- $restTls := .Values.apis.rest.security.tls -}}
{{- $mutualAuth := dig "mutualAuth" $globalTls.mutualAuth $restTls -}}
{{- $ag := $globalTls.autoGenerated -}}
{{- if and $globalTls.enabled (not $ag.enabled) -}}
    {{- if not $restTls.existingSecret -}}
        {{- fail "tls: apis.rest.security.tls.existingSecret must be set when tls.autoGenerated.enabled is false" -}}
    {{- end -}}
    {{- if $mutualAuth -}}
        {{- $clients := $restTls.clients -}}
        {{- if not $clients.csiController.existingSecret -}}
            {{- fail "tls: apis.rest.security.tls.clients.csiController.existingSecret must be set when tls.autoGenerated.enabled is false and mutualAuth is true" -}}
        {{- end -}}
        {{- if not $clients.csiNode.existingSecret -}}
            {{- fail "tls: apis.rest.security.tls.clients.csiNode.existingSecret must be set when tls.autoGenerated.enabled is false and mutualAuth is true" -}}
        {{- end -}}
        {{- if not $clients.callhome.existingSecret -}}
            {{- fail "tls: apis.rest.security.tls.clients.callhome.existingSecret must be set when tls.autoGenerated.enabled is false and mutualAuth is true" -}}
        {{- end -}}
        {{- if not $clients.metricsExporter.existingSecret -}}
            {{- fail "tls: apis.rest.security.tls.clients.metricsExporter.existingSecret must be set when tls.autoGenerated.enabled is false and mutualAuth is true" -}}
        {{- end -}}
        {{- if not $clients.diskpoolOperator.existingSecret -}}
            {{- fail "tls: apis.rest.security.tls.clients.diskpoolOperator.existingSecret must be set when tls.autoGenerated.enabled is false and mutualAuth is true" -}}
        {{- end -}}
        {{- if not $clients.plugin.existingSecret -}}
            {{- fail "tls: apis.rest.security.tls.clients.plugin.existingSecret must be set when tls.autoGenerated.enabled is false and mutualAuth is true" -}}
        {{- end -}}
    {{- end -}}
{{- else -}}
    {{- if not (has $ag.engine (list "pod" "helm" "cert-manager")) -}}
        {{- fail (printf "tls.autoGenerated.engine must be pod, helm, or cert-manager (got: %q)" $ag.engine) -}}
    {{- end -}}
    {{- if and $globalTls.enabled (eq $ag.engine "pod") $mutualAuth -}}
        {{- fail "tls: engine=pod cannot be used with mutualAuth — pod TLS uses a transient cert with no client verification" -}}
    {{- end -}}
    {{- if and $globalTls.enabled (eq $ag.engine "cert-manager") -}}
        {{- if not (.Capabilities.APIVersions.Has "cert-manager.io/v1") -}}
            {{- fail "tls.autoGenerated.engine=cert-manager requires cert-manager CRDs. Install cert-manager first." -}}
        {{- end -}}
        {{- if and $ag.certManager.existingIssuer (not (or (eq $ag.certManager.existingIssuerKind "Issuer") (eq $ag.certManager.existingIssuerKind "ClusterIssuer"))) -}}
            {{- fail (printf "tls.autoGenerated.certManager.existingIssuerKind must be Issuer or ClusterIssuer (got: %q)" $ag.certManager.existingIssuerKind) -}}
        {{- end -}}
    {{- end -}}
{{- end -}}
{{- end -}}

{{/*
Returns the REST API endpoint URL for in-cluster clients.
TLS disabled: plain HTTP on port 8081. TLS enabled: HTTPS on port 8080.
*/}}
{{- define "rest_api_endpoint" -}}
{{- if .Values.security.tls.enabled -}}
    {{- printf "https://%s-api-rest:8080" .Release.Name -}}
{{- else -}}
    {{- printf "http://%s-api-rest:8081" .Release.Name -}}
{{- end -}}
{{- end -}}

{{/*
Returns the Secret name for the api-rest server TLS certificate and key.
Used when tls.enabled=true and engine != pod.
*/}}
{{- define "rest_api_tls_secret_name" -}}
{{- $globalTls := .Values.security.tls -}}
{{- $restTls := .Values.apis.rest.security.tls -}}
{{- if $globalTls.enabled }}
    {{- if not $globalTls.autoGenerated.enabled -}}
        {{- $restTls.existingSecret -}}
    {{- else if eq $globalTls.autoGenerated.engine "cert-manager" -}}
        {{- $restTls.certManager.secretName | default (printf "%s-api-rest-crt" .Release.Name) -}}
    {{- else -}}
        {{- printf "%s-api-rest-crt" .Release.Name -}}
    {{- end -}}
{{- end -}}
{{- end -}}

{{- define "rest_api_tls_csi_controller_client_secret_name" -}}
{{- $globalTls := .Values.security.tls -}}
{{- $client := .Values.apis.rest.security.tls.clients.csiController -}}
{{- if $globalTls.enabled -}}
    {{- if not $globalTls.autoGenerated.enabled -}}
        {{- $client.existingSecret -}}
    {{- else if eq $globalTls.autoGenerated.engine "cert-manager" -}}
        {{- $client.certManager.secretName | default (printf "%s-api-rest-csi-controller-crt" .Release.Name) -}}
    {{- else -}}
        {{- printf "%s-api-rest-csi-controller-crt" .Release.Name -}}
    {{- end -}}
{{- end -}}
{{- end -}}

{{- define "rest_api_tls_csi_node_client_secret_name" -}}
{{- $globalTls := .Values.security.tls -}}
{{- $client := .Values.apis.rest.security.tls.clients.csiNode -}}
{{- if $globalTls.enabled -}}
    {{- if not $globalTls.autoGenerated.enabled -}}
        {{- $client.existingSecret -}}
    {{- else if eq $globalTls.autoGenerated.engine "cert-manager" -}}
        {{- $client.certManager.secretName | default (printf "%s-api-rest-csi-node-crt" .Release.Name) -}}
    {{- else -}}
        {{- printf "%s-api-rest-csi-node-crt" .Release.Name -}}
    {{- end -}}
{{- end -}}
{{- end -}}

{{- define "rest_api_tls_callhome_client_secret_name" -}}
{{- $globalTls := .Values.security.tls -}}
{{- $client := .Values.apis.rest.security.tls.clients.callhome -}}
{{- if $globalTls.enabled -}}
    {{- if not $globalTls.autoGenerated.enabled -}}
        {{- $client.existingSecret -}}
    {{- else if eq $globalTls.autoGenerated.engine "cert-manager" -}}
        {{- $client.certManager.secretName | default (printf "%s-api-rest-callhome-crt" .Release.Name) -}}
    {{- else -}}
        {{- printf "%s-api-rest-callhome-crt" .Release.Name -}}
    {{- end -}}
{{- end -}}
{{- end -}}

{{- define "rest_api_tls_metrics_exporter_client_secret_name" -}}
{{- $globalTls := .Values.security.tls -}}
{{- $client := .Values.apis.rest.security.tls.clients.metricsExporter -}}
{{- if $globalTls.enabled -}}
    {{- if not $globalTls.autoGenerated.enabled -}}
        {{- $client.existingSecret -}}
    {{- else if eq $globalTls.autoGenerated.engine "cert-manager" -}}
        {{- $client.certManager.secretName | default (printf "%s-api-rest-metrics-exporter-crt" .Release.Name) -}}
    {{- else -}}
        {{- printf "%s-api-rest-metrics-exporter-crt" .Release.Name -}}
    {{- end -}}
{{- end -}}
{{- end -}}

{{- define "rest_api_tls_diskpool_operator_client_secret_name" -}}
{{- $globalTls := .Values.security.tls -}}
{{- $client := .Values.apis.rest.security.tls.clients.diskpoolOperator -}}
{{- if $globalTls.enabled -}}
    {{- if not $globalTls.autoGenerated.enabled -}}
        {{- $client.existingSecret -}}
    {{- else if eq $globalTls.autoGenerated.engine "cert-manager" -}}
        {{- $client.certManager.secretName | default (printf "%s-api-rest-diskpool-operator-crt" .Release.Name) -}}
    {{- else -}}
        {{- printf "%s-api-rest-diskpool-operator-crt" .Release.Name -}}
    {{- end -}}
{{- end -}}
{{- end -}}

{{- define "rest_api_tls_plugin_client_secret_name" -}}
{{- $globalTls := .Values.security.tls -}}
{{- $client := .Values.apis.rest.security.tls.clients.plugin -}}
{{- if $globalTls.enabled -}}
    {{- if not $globalTls.autoGenerated.enabled -}}
        {{- $client.existingSecret -}}
    {{- else if eq $globalTls.autoGenerated.engine "cert-manager" -}}
        {{- $client.certManager.secretName | default (printf "%s-api-rest-plugin-crt" .Release.Name) -}}
    {{- else -}}
        {{- printf "%s-api-rest-plugin-crt" .Release.Name -}}
    {{- end -}}
{{- end -}}
{{- end -}}

{{/*
Returns the pod annotation value for kubectl-plugin TLS auto-discovery.
Format: "{mode}:{secretName}". Empty when TLS is disabled or engine is pod.
  mtls mode:         references the plugin client cert (used for port-forward with localhost SAN).
  server-verify mode: references the server cert (plugin only needs ca.crt; no client cert exists).
Usage: {{ include "rest_api_tls_annotation" . }}
*/}}
{{- define "rest_api_tls_annotation" -}}
{{- $globalTls := .Values.security.tls -}}
{{- $restTls := .Values.apis.rest.security.tls -}}
{{- $mutualAuth := dig "mutualAuth" $globalTls.mutualAuth $restTls -}}
{{- if and $globalTls.enabled (or (not $globalTls.autoGenerated.enabled) (ne $globalTls.autoGenerated.engine "pod")) -}}
    {{- if $mutualAuth -}}
        {{- printf "mtls:%s" (include "rest_api_tls_plugin_client_secret_name" .) -}}
    {{- else -}}
        {{- printf "server-verify:%s" (include "rest_api_tls_secret_name" .) -}}
    {{- end -}}
{{- else if and $globalTls.enabled $globalTls.autoGenerated.enabled (eq $globalTls.autoGenerated.engine "pod") }}
    {{- print "auto" -}}
{{- end -}}
{{- end -}}

{{/*
Returns TLS CLI args for in-cluster REST API clients (metrics-exporter, callhome, CSI, pool operator).
  disabled or engine=pod: empty
  server-only (not mutualAuth):  --tls-ca-file
  mutualAuth:                    --tls-ca-file + --tls-cert-file + --tls-key-file
Usage: {{- include "rest_api_tls_client_args" . | nindent N }}
*/}}
{{- define "rest_api_tls_client_args" -}}
{{- $globalTls := .Values.security.tls -}}
{{- $restTls := .Values.apis.rest.security.tls -}}
{{- $mutualAuth := dig "mutualAuth" $globalTls.mutualAuth $restTls -}}
{{- if and $globalTls.enabled (or (not $globalTls.autoGenerated.enabled) (ne $globalTls.autoGenerated.engine "pod")) -}}
    {{- if $mutualAuth }}
- "--tls-ca-file=/etc/tls/ca.crt"
- "--tls-cert-file=/etc/tls/tls.crt"
- "--tls-key-file=/etc/tls/tls.key"
    {{- else }}
- "--tls-ca-file=/etc/tls/ca.crt"
    {{- end -}}
{{- end -}}
{{- end -}}

{{/*
Returns the volumeMount block for in-cluster REST API clients.
Usage: {{- include "rest_api_tls_client_volume_mount" . | nindent N }}
*/}}
{{- define "rest_api_tls_client_volume_mount" -}}
{{- $globalTls := .Values.security.tls -}}
{{- if and $globalTls.enabled (or (not $globalTls.autoGenerated.enabled) (ne $globalTls.autoGenerated.engine "pod")) -}}
- name: api-rest-tls
  mountPath: /etc/tls
  readOnly: true
{{- end -}}
{{- end -}}

{{/*
Returns the volume declaration for in-cluster REST API clients.
Accepts either the plain chart context (.) for backwards compat, or a dict of the form:
  (dict "ctx" . "secretName" "the-per-client-secret-name")
When secretName is provided and mutualAuth is true, it is used directly.
Otherwise the server TLS secret (ca.crt only) is used.
Usage: {{- include "rest_api_tls_client_volume" . | nindent N }}
       {{- include "rest_api_tls_client_volume" (dict "ctx" . "secretName" (include "rest_api_tls_csi_controller_client_secret_name" .)) | nindent N }}
*/}}
{{- define "rest_api_tls_client_volume" -}}
{{- $ctx := . -}}
{{- $secretName := "" -}}
{{- if hasKey . "ctx" -}}
    {{- $ctx = .ctx -}}
    {{- $secretName = .secretName -}}
{{- end -}}
{{- $globalTls := $ctx.Values.security.tls -}}
{{- $restTls := $ctx.Values.apis.rest.security.tls -}}
{{- $mutualAuth := dig "mutualAuth" $globalTls.mutualAuth $restTls -}}
{{- if and $globalTls.enabled (or (not $globalTls.autoGenerated.enabled) (ne $globalTls.autoGenerated.engine "pod")) -}}
- name: api-rest-tls
  secret:
    secretName: {{ if and $mutualAuth $secretName -}}
      {{- $secretName }}
    {{- else -}}
      {{- include "rest_api_tls_secret_name" $ctx }}
    {{- end }}
{{- end -}}
{{- end -}}

{{/*
Resolves and emits a single helm-engine TLS Secret.
Reuses the existing secret if it was signed by the current CA; otherwise generates a fresh leaf cert.
Args (dict):
  ctx          — root context (.)
  ca           — CA struct from genCA / buildCustomCert
  caExists     — bool: whether the CA secret was found in the cluster
  certDuration — leaf cert validity in days
  secretName   — Kubernetes Secret name to look up and emit
  cn           — X.509 common name for the generated cert
  dnsSANs      — list of DNS SANs (empty list for client certs, ["localhost"] for plugin)
  app          — value for the "app" label
*/}}
{{- define "tls_helm_leaf_secret" -}}
{{- $ctx := .ctx -}}
{{- $ca := .ca -}}
{{- $existing := lookup "v1" "Secret" $ctx.Release.Namespace .secretName -}}
{{- $existingCaCrt := "" -}}
{{- if and $existing $existing.data (index $existing.data "ca.crt") -}}
  {{- $existingCaCrt = index $existing.data "ca.crt" -}}
{{- end -}}
{{- $cert := "" -}}
{{- $key := "" -}}
{{- if and .caExists $existing $existing.data (index $existing.data "tls.crt") (eq $existingCaCrt ($ca.Cert | b64enc)) -}}
  {{- $cert = index $existing.data "tls.crt" | b64dec -}}
  {{- $key = index $existing.data "tls.key" | b64dec -}}
{{- else -}}
  {{- $g := genSignedCert .cn (list) .dnsSANs .certDuration $ca -}}
  {{- $cert = $g.Cert -}}
  {{- $key = $g.Key -}}
{{- end -}}
---
apiVersion: v1
kind: Secret
metadata:
  name: {{ .secretName }}
  namespace: {{ $ctx.Release.Namespace }}
  labels:
    app: {{ .app }}
    {{ include "label_prefix" $ctx }}/release: {{ $ctx.Release.Name }}
    {{ include "label_prefix" $ctx }}/version: {{ $ctx.Chart.Version }}
type: kubernetes.io/tls
data:
  ca.crt: {{ $ca.Cert | b64enc }}
  tls.crt: {{ $cert | b64enc }}
  tls.key: {{ $key | b64enc }}
{{- end -}}
