use crate::{
    common::{
        constants::{
            KUBE_API_PAGE_SIZE, TWO_DOT_FIVE, TWO_DOT_FOUR, TWO_DOT_ONE, TWO_DOT_O_RC_ONE,
            TWO_DOT_SIX, TWO_DOT_THREE,
        },
        error::{
            DeserializePromtailExtraConfig, ListCrds, Result, SemverParse,
            SerializePromtailConfigClientToJson, SerializePromtailExtraConfigToJson,
            SerializePromtailInitContainerToJson,
        },
        file::write_to_tempfile,
        kube_client as KubeClient,
    },
    helm::{
        chart::{CoreValues, PromtailConfigClient},
        yaml::yq::{YamlKey, YqV4},
    },
};
use kube::{api::ListParams, ResourceExt};
use semver::Version;
use snafu::ResultExt;
use std::{collections::HashMap, path::Path};
use tempfile::NamedTempFile as TempFile;

/// This compiles all of the helm values options to be passed during the helm chart upgrade.
/// The helm-chart to helm-chart upgrade has two sets of helm-values. They should both be
/// deserialized prior to calling this.
/// Parameters:
///     source_version: &Version --> The helm chart version of the source helm chart. Because the
/// source chart is already installed, this value may be deserialized as a helm releaseElement
/// (https://github.com/helm/helm/blob/v3.13.2/cmd/helm/list.go#L141) member, from a `helm list`
/// output. The 'chart' may then be split into chart-name and chart-version.
///     target_version: &Version --> The helm chart version of the target helm chart. The target
/// helm chart should be available in the local filesystem. The value may be picked out from the
/// Chart.yaml file (version, not appVersion) in the helm chart directory.
///     source_values: &CoreValues --> This is the deserialized struct generated from the helm
/// values of the source chart. The values are sourced from `helm get values --all -o yaml`
///     target_values: &CoreValues --> This is the deserialized struct generated from the
/// locally available target helm chart's values.yaml file.
///     source_values_buf: Vec<u8> --> This is the value that is read from the `helm get values
/// --all -o yaml` output. This is required in a buffer so that it may be written to a file and
/// used with the yq-go binary.
///     target_values_filepath --> This is simply the path to the values.yaml file for the target
/// helm chart, which is available locally.
///     chart_dir --> This is the path to a directory that yq-go may use to write its output file
/// into. The output file will be a merged values.yaml with special values set as per requirement
/// (based on source_version and target_version).
pub(crate) async fn generate_values_yaml_file<P, Q>(
    source_version: &Version,
    target_version: &Version,
    source_values: &CoreValues,
    target_values: &CoreValues,
    source_values_buf: Vec<u8>,
    target_values_filepath: P,
    chart_dir: Q,
) -> Result<TempFile>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    // Write the source_values buffer fetched from the installed helm release to a temporary file.
    let source_values_file: TempFile =
        write_to_tempfile(Some(chart_dir.as_ref()), source_values_buf.as_slice())?;

    // Resultant values yaml for helm upgrade command.
    // Merge the source values with the target values.
    let yq = YqV4::new()?;
    let upgrade_values_yaml =
        yq.merge_files(source_values_file.path(), target_values_filepath.as_ref())?;
    let upgrade_values_file: TempFile =
        write_to_tempfile(Some(chart_dir.as_ref()), upgrade_values_yaml.as_slice())?;

    // Not using semver::VersionReq because expressions like '>=2.1.0' don't include
    // 2.3.0-rc.0. 2.3.0, 2.4.0, etc. are supported. So, not using VersionReq in the
    // below comparisons because of this.

    // Specific special-case values for version 2.0.x.
    let two_dot_o_rc_zero = Version::parse(TWO_DOT_O_RC_ONE).context(SemverParse {
        version_string: TWO_DOT_O_RC_ONE.to_string(),
    })?;
    let two_dot_one = Version::parse(TWO_DOT_ONE).context(SemverParse {
        version_string: TWO_DOT_ONE.to_string(),
    })?;
    if source_version.ge(&two_dot_o_rc_zero) && source_version.lt(&two_dot_one) {
        let log_level_to_replace = "info,io_engine=info";

        if source_values.io_engine_log_level().eq(log_level_to_replace)
            && target_values.io_engine_log_level().ne(log_level_to_replace)
        {
            yq.set_literal_value(
                YamlKey::try_from(".io_engine.logLevel")?,
                target_values.io_engine_log_level(),
                upgrade_values_file.path(),
            )?;
        }
    }

    // Specific special-case values for to-version >=2.1.x.
    if target_version.ge(&two_dot_one) {
        // RepoTags fields will also be set to the values found in the target helm values file
        // (low_priority file). This is so integration tests which use specific repo commits can
        // upgrade to a custom helm chart.
        yq.set_literal_value(
            YamlKey::try_from(".image.repoTags.controlPlane")?,
            target_values.control_plane_repotag(),
            upgrade_values_file.path(),
        )?;
        yq.set_literal_value(
            YamlKey::try_from(".image.repoTags.dataPlane")?,
            target_values.data_plane_repotag(),
            upgrade_values_file.path(),
        )?;
        yq.set_literal_value(
            YamlKey::try_from(".image.repoTags.extensions")?,
            target_values.extensions_repotag(),
            upgrade_values_file.path(),
        )?;
    }

    // Specific special-case values for version 2.3.x.
    let two_dot_three = Version::parse(TWO_DOT_THREE).context(SemverParse {
        version_string: TWO_DOT_THREE.to_string(),
    })?;
    let two_dot_four = Version::parse(TWO_DOT_FOUR).context(SemverParse {
        version_string: TWO_DOT_FOUR.to_string(),
    })?;
    if source_version.ge(&two_dot_three)
        && source_version.lt(&two_dot_four)
        && source_values
            .eventing_enabled()
            .ne(&target_values.eventing_enabled())
    {
        yq.set_literal_value(
            YamlKey::try_from(".eventing.enabled")?,
            target_values.eventing_enabled(),
            upgrade_values_file.path(),
        )?;
    }

    // Special-case values for 2.5.x.
    let two_dot_five = Version::parse(TWO_DOT_FIVE).context(SemverParse {
        version_string: TWO_DOT_FIVE.to_string(),
    })?;
    if source_version.ge(&two_dot_o_rc_zero) && source_version.lt(&two_dot_five) {
        // promtail
        let scrape_configs_to_replace = r"- job_name: {{ .Release.Name }}-pods-name
  pipeline_stages:
    - docker: {}
  kubernetes_sd_configs:
  - role: pod
  relabel_configs:
  - source_labels:
    - __meta_kubernetes_pod_node_name
    target_label: hostname
    action: replace
  - action: labelmap
    regex: __meta_kubernetes_pod_label_(.+)
  - action: keep
    source_labels:
    - __meta_kubernetes_pod_label_openebs_io_logging
    regex: true
    target_label: {{ .Release.Name }}_component
  - action: replace
    replacement: $1
    separator: /
    source_labels:
    - __meta_kubernetes_namespace
    target_label: job
  - action: replace
    source_labels:
    - __meta_kubernetes_pod_name
    target_label: pod
  - action: replace
    source_labels:
    - __meta_kubernetes_pod_container_name
    target_label: container
  - replacement: /var/log/pods/*$1/*.log
    separator: /
    source_labels:
    - __meta_kubernetes_pod_uid
    - __meta_kubernetes_pod_container_name
    target_label: __path__
";
        if source_values
            .loki_stack_promtail_scrape_configs()
            .eq(scrape_configs_to_replace)
            && target_values
                .loki_stack_promtail_scrape_configs()
                .ne(scrape_configs_to_replace)
        {
            yq.set_literal_value(
                YamlKey::try_from(".loki-stack.promtail.config.snippets.scrapeConfigs")?,
                target_values.loki_stack_promtail_scrape_configs(),
                upgrade_values_file.path(),
            )?;
        }

        // io_timeout
        let io_timeout_to_replace = "30";
        if source_values
            .csi_node_nvme_io_timeout()
            .eq(io_timeout_to_replace)
            && target_values
                .csi_node_nvme_io_timeout()
                .ne(io_timeout_to_replace)
        {
            yq.set_literal_value(
                YamlKey::try_from(".csi.node.nvme.io_timeout")?,
                target_values.csi_node_nvme_io_timeout(),
                upgrade_values_file.path(),
            )?;
        }
    }

    // Special-case values for 2.6.x.
    let two_dot_six = Version::parse(TWO_DOT_SIX).context(SemverParse {
        version_string: TWO_DOT_SIX.to_string(),
    })?;
    if source_version.ge(&two_dot_o_rc_zero) && source_version.lt(&two_dot_six) {
        // Update localpv-provisioner helm chart.
        // This change is meant for versions from 2.0.0 to 2.4.0. However, this code wasn't checked
        // into 2.5.0, and likely users of upgrade-job 2.5.0 are using the localpv image tag
        // from 2.4.0 (i.e. 3.4.0) with the 3.5.0 localpv helm chart. So these options should
        // also be set for source version 2.5.0.
        let localpv_version_to_replace = "3.4.0";
        if source_values
            .localpv_release_version()
            .eq(localpv_version_to_replace)
            && target_values
                .localpv_release_version()
                .ne(localpv_version_to_replace)
        {
            yq.set_literal_value(
                YamlKey::try_from(".localpv-provisioner.release.version")?,
                target_values.localpv_release_version(),
                upgrade_values_file.path(),
            )?;
            yq.set_literal_value(
                YamlKey::try_from(".localpv-provisioner.localpv.image.tag")?,
                target_values.localpv_provisioner_image_tag(),
                upgrade_values_file.path(),
            )?;
            yq.set_literal_value(
                YamlKey::try_from(".localpv-provisioner.helperPod.image.tag")?,
                target_values.localpv_helper_image_tag(),
                upgrade_values_file.path(),
            )?;
        }

        // Switch out image tag for the latest one.
        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.loki.image.tag")?,
            target_values.loki_stack_loki_image_tag(),
            upgrade_values_file.path(),
        )?;
        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.filebeat.imageTag")?,
            target_values.filebeat_image_tag(),
            upgrade_values_file.path(),
        )?;
        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.logstash.imageTag")?,
            target_values.logstash_image_tag(),
            upgrade_values_file.path(),
        )?;
        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.grafana.downloadDashboardsImage.tag")?,
            target_values.grafana_download_dashboards_image_tag(),
            upgrade_values_file.path(),
        )?;
        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.grafana.image.tag")?,
            target_values.grafana_image_tag(),
            upgrade_values_file.path(),
        )?;
        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.grafana.sidecar.image.tag")?,
            target_values.grafana_sidecar_image_tag(),
            upgrade_values_file.path(),
        )?;
        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.prometheus.alertmanager.image.tag")?,
            target_values.prometheus_alertmanager_image_tag(),
            upgrade_values_file.path(),
        )?;
        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.prometheus.nodeExporter.image.tag")?,
            target_values.prometheus_node_exporter_image_tag(),
            upgrade_values_file.path(),
        )?;
        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.prometheus.pushgateway.image.tag")?,
            target_values.prometheus_pushgateway_image_tag(),
            upgrade_values_file.path(),
        )?;
        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.prometheus.server.image.tag")?,
            target_values.prometheus_server_image_tag(),
            upgrade_values_file.path(),
        )?;

        // Delete deprecated objects.
        yq.delete_object(
            YamlKey::try_from(".loki-stack.loki.config.ingester.lifecycler.ring.kvstore")?,
            upgrade_values_file.path(),
        )?;
        yq.delete_object(
            YamlKey::try_from(".loki-stack.promtail.config.snippets.extraClientConfigs")?,
            upgrade_values_file.path(),
        )?;
        yq.delete_object(
            YamlKey::try_from(".loki-stack.promtail.initContainer")?,
            upgrade_values_file.path(),
        )?;

        loki_address_to_clients(source_values, upgrade_values_file.path(), &yq)?;

        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.promtail.config.file")?,
            target_values.loki_stack_promtail_config_file(),
            upgrade_values_file.path(),
        )?;
        for container in target_values.promtail_init_container() {
            let init_container = serde_json::to_string(&container).context(
                SerializePromtailInitContainerToJson {
                    object: container.clone(),
                },
            )?;
            yq.append_to_object(
                YamlKey::try_from(".loki-stack.promtail.initContainer")?,
                init_container,
                upgrade_values_file.path(),
            )?;
        }
        yq.set_literal_value(
            YamlKey::try_from(".loki-stack.promtail.readinessProbe.httpGet.path")?,
            target_values.promtail_readiness_probe_http_get_path(),
            upgrade_values_file.path(),
        )?;

        // This helm value key was changed:
        // Ref: https://github.com/openebs/mayastor-extensions/pull/419
        yq.set_literal_value(
            YamlKey::try_from(".base.logging.silenceLevel")?,
            source_values.deprecated_log_silence_level(),
            upgrade_values_file.path(),
        )?;
        yq.delete_object(
            YamlKey::try_from(".base.logSilenceLevel")?,
            upgrade_values_file.path(),
        )?;

        // This is a fix for a typo in the .csi.node.pluginMounthPath key.
        // It was fixed, and the key now is called .csi.node.pluginMountPath.
        yq.set_literal_value(
            YamlKey::try_from(".csi.node.pluginMountPath")?,
            source_values.deprecated_node_csi_mount_path(),
            upgrade_values_file.path(),
        )?;
        yq.delete_object(
            YamlKey::try_from(".csi.node.pluginMounthPath")?,
            upgrade_values_file.path(),
        )?;

        // This sets the image tag for the jaeger-operator. This is required for the
        // jaeger-operator dependency update from 2.50.0 to 2.50.1.
        yq.set_literal_value(
            YamlKey::try_from(".jaeger-operator.image.tag")?,
            source_values.jaeger_operator_image_tag(),
            upgrade_values_file.path(),
        )?;
    }

    // Default options.
    // Image tag is set because the high_priority file is the user's source options file.
    // The target's image tag needs to be set for PRODUCT upgrade.
    yq.set_literal_value(
        YamlKey::try_from(".image.tag")?,
        target_values.image_tag(),
        upgrade_values_file.path(),
    )?;

    // The CSI sidecar images need to always be the versions set on the chart by default.
    yq.set_literal_value(
        YamlKey::try_from(".csi.image.provisionerTag")?,
        target_values.csi_provisioner_image_tag(),
        upgrade_values_file.path(),
    )?;
    yq.set_literal_value(
        YamlKey::try_from(".csi.image.attacherTag")?,
        target_values.csi_attacher_image_tag(),
        upgrade_values_file.path(),
    )?;
    yq.set_literal_value(
        YamlKey::try_from(".csi.image.snapshotterTag")?,
        target_values.csi_snapshotter_image_tag(),
        upgrade_values_file.path(),
    )?;
    yq.set_literal_value(
        YamlKey::try_from(".csi.image.snapshotControllerTag")?,
        target_values.csi_snapshot_controller_image_tag(),
        upgrade_values_file.path(),
    )?;
    yq.set_literal_value(
        YamlKey::try_from(".csi.image.registrarTag")?,
        target_values.csi_node_driver_registrar_image_tag(),
        upgrade_values_file.path(),
    )?;
    yq.set_literal_value(
        YamlKey::try_from(".csi.image.resizerTag")?,
        target_values.csi_resizer_image_tag(),
        upgrade_values_file.path(),
    )?;

    // Disable CRD installation in case they already exist using helm values.
    safe_crd_install(upgrade_values_file.path(), &yq).await?;

    // helm upgrade .. --set image.tag=<version> --set image.repoTags.controlPlane= --set
    // image.repoTags.dataPlane= --set image.repoTags.extensions=

    Ok(upgrade_values_file)
}

/// Converts config.lokiAddress and config.snippets.extraClientConfigs from the promtail helm chart
/// v3.11.0 to config.clients[] which is compatible with promtail helm chart v6.13.1.
fn loki_address_to_clients(
    source_values: &CoreValues,
    upgrade_values_filepath: &Path,
    yq: &YqV4,
) -> Result<()> {
    let promtail_config_clients_yaml_key =
        YamlKey::try_from(".loki-stack.promtail.config.clients")?;
    // Delete existing array, if any. The merge_files() should have added it with the default value
    // set.
    yq.delete_object(
        promtail_config_clients_yaml_key.clone(),
        upgrade_values_filepath,
    )?;
    let loki_address = source_values.loki_stack_promtail_loki_address();
    let promtail_config_client = PromtailConfigClient::with_url(loki_address);
    // Serializing to JSON, because the yq command requires the input in JSON.
    let promtail_config_client = serde_json::to_string(&promtail_config_client).context(
        SerializePromtailConfigClientToJson {
            object: promtail_config_client,
        },
    )?;
    yq.append_to_array(
        promtail_config_clients_yaml_key,
        promtail_config_client,
        upgrade_values_filepath,
    )?;
    // Merge the extraClientConfigs from the promtail v3.11.0 chart to the v6.13.1 chart's
    // config.clients block. Ref: https://github.com/grafana/helm-charts/issues/1214
    // Ref: https://github.com/grafana/helm-charts/pull/1425
    if !source_values.promtail_extra_client_configs().is_empty() {
        // Converting the YAML to a JSON because the yq command requires the object input as a JSON.
        let promtail_extra_client_config: serde_json::Value = serde_yaml::from_str(
            source_values.promtail_extra_client_configs(),
        )
        .context(DeserializePromtailExtraConfig {
            config: source_values.promtail_extra_client_configs().to_string(),
        })?;
        let promtail_extra_client_config = serde_json::to_string(&promtail_extra_client_config)
            .context(SerializePromtailExtraConfigToJson {
                config: promtail_extra_client_config,
            })?;

        yq.append_to_object(
            YamlKey::try_from(".loki-stack.promtail.config.clients[0]")?,
            promtail_extra_client_config,
            upgrade_values_filepath,
        )?;
    }

    // Cleanup config.snippets.extraClientConfig from the promtail chart.
    yq.delete_object(
        YamlKey::try_from(".loki-stack.promtail.config.snippets.extraClientConfigs")?,
        upgrade_values_filepath,
    )?;

    // Cleanup config.lokiAddress from the promtail chart.
    yq.delete_object(
        YamlKey::try_from(".loki-stack.promtail.config.lokiAddress")?,
        upgrade_values_filepath,
    )?;

    Ok(())
}

/// Use pre-defined helm chart templating to disable CRD installation if they already exist.
async fn safe_crd_install(upgrade_values_filepath: &Path, yq: &YqV4) -> Result<()> {
    let mut crd_set_to_helm_toggle: HashMap<Vec<&str>, YamlKey> = HashMap::new();
    // These 3 CRDs usually exist together.
    crd_set_to_helm_toggle.insert(
        vec![
            "volumesnapshotclasses.snapshot.storage.k8s.io",
            "volumesnapshotcontents.snapshot.storage.k8s.io",
            "volumesnapshots.snapshot.storage.k8s.io",
        ],
        YamlKey::try_from(".crds.csi.volumeSnapshots.enabled")?,
    );
    crd_set_to_helm_toggle.insert(
        vec!["jaegers.jaegertracing.io"],
        YamlKey::try_from(".crds.jaeger.enabled")?,
    );

    let crds_api = KubeClient::crds_api().await?;
    let mut all_crd_names: Vec<String> = Vec::with_capacity(KUBE_API_PAGE_SIZE as usize);
    let mut list_params = ListParams::default().limit(KUBE_API_PAGE_SIZE);

    loop {
        let crd_list = crds_api
            .list_metadata(&list_params)
            .await
            .context(ListCrds)?;

        all_crd_names = all_crd_names
            .into_iter()
            .chain(crd_list.iter().map(|metadata| metadata.name_unchecked()))
            .collect();

        match crd_list.metadata.continue_ {
            Some(token) => {
                list_params = list_params.continue_token(token.as_str());
            }
            None => break,
        }
    }

    for (crd_set, helm_toggle) in crd_set_to_helm_toggle.into_iter() {
        // Uses an OR logical check to disable set installation, i.e. disable if
        // at least one exists. Does not make sure if all exist.
        if all_crd_names
            .iter()
            .any(|name| crd_set.contains(&name.as_str()))
        {
            yq.set_literal_value(helm_toggle, false, upgrade_values_filepath)?
        }
    }

    Ok(())
}
