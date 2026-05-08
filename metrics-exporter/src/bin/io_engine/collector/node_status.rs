use openapi::models::{CordonDrainState, Node, NodeStatus};
use prometheus::{
    core::{Collector, Desc},
    proto, GaugeVec, Opts,
};

/// Returns true if the node is online.
fn is_online(node: &Node) -> bool {
    node.state
        .as_ref()
        .map(|s| matches!(s.status, NodeStatus::Online))
        .unwrap_or(false)
}

/// Returns true if the node is draining or drained.
fn is_draining(node: &Node) -> bool {
    node.spec
        .as_ref()
        .and_then(|s| s.cordondrainstate.as_ref())
        .map(|cds| {
            matches!(
                cds,
                CordonDrainState::drainingstate(_) | CordonDrainState::drainedstate(_)
            )
        })
        .unwrap_or(false)
}

/// Returns true if the node is cordoned (including implicit cordon from drain).
fn is_cordoned(node: &Node) -> bool {
    is_draining(node)
        || node
            .spec
            .as_ref()
            .and_then(|s| s.cordondrainstate.as_ref())
            .map(|cds| matches!(cds, CordonDrainState::cordonedstate(_)))
            .unwrap_or(false)
}

/// Collector for node status metrics.
pub(crate) struct NodeStatusCollector {
    /// Node data fetched at scrape time (None if unavailable).
    node: Option<Node>,
    /// Gauge for node online status (0/1).
    node_online: GaugeVec,
    /// Gauge for node cordoned status (0/1).
    node_cordoned: GaugeVec,
    /// Gauge for node draining status (0/1).
    node_draining: GaugeVec,
    /// Descriptors for Prometheus.
    descs: Vec<Desc>,
}

impl NodeStatusCollector {
    /// Create a new NodeStatusCollector from node data fetched at scrape time.
    pub(crate) fn new(node: Option<Node>) -> Self {
        let mut descs = Vec::new();

        let node_online = GaugeVec::new(
            Opts::new("node_online", "Indicates if Mayastor node is online")
                .subsystem("mayastor")
                .variable_labels(vec!["node_id".to_string()]),
            &["node_id"],
        )
        .expect("Unable to create gauge metric for node_online");
        descs.extend(node_online.desc().into_iter().cloned());

        let node_cordoned = GaugeVec::new(
            Opts::new("node_cordoned", "Indicates if Mayastor node is cordoned")
                .subsystem("mayastor")
                .variable_labels(vec!["node_id".to_string()]),
            &["node_id"],
        )
        .expect("Unable to create gauge metric for node_cordoned");
        descs.extend(node_cordoned.desc().into_iter().cloned());

        let node_draining = GaugeVec::new(
            Opts::new("node_draining", "Indicates if Mayastor node is draining")
                .subsystem("mayastor")
                .variable_labels(vec!["node_id".to_string()]),
            &["node_id"],
        )
        .expect("Unable to create gauge metric for node_draining");
        descs.extend(node_draining.desc().into_iter().cloned());

        Self {
            node,
            node_online,
            node_cordoned,
            node_draining,
            descs,
        }
    }

    /// Update metrics from node data fetched at scrape time.
    fn update_metrics(&self) {
        let node = match &self.node {
            Some(n) => n,
            None => return,
        };

        let node_id = node.id.as_str();

        self.node_online
            .with_label_values(&[node_id])
            .set(if is_online(node) { 1.0 } else { 0.0 });

        self.node_cordoned
            .with_label_values(&[node_id])
            .set(if is_cordoned(node) { 1.0 } else { 0.0 });

        self.node_draining
            .with_label_values(&[node_id])
            .set(if is_draining(node) { 1.0 } else { 0.0 });
    }
}

impl Collector for NodeStatusCollector {
    fn desc(&self) -> Vec<&Desc> {
        self.descs.iter().collect()
    }

    fn collect(&self) -> Vec<proto::MetricFamily> {
        self.update_metrics();

        let mut metrics = Vec::new();
        metrics.extend(self.node_online.collect());
        metrics.extend(self.node_cordoned.collect());
        metrics.extend(self.node_draining.collect());
        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openapi::models::{CordonedState, DrainState, Node, NodeSpec, NodeState};
    use prometheus::core::Collector;

    /// Create a test node with the specified status and cordon/drain state.
    fn create_test_node(id: &str, online: bool, cordon_drain: Option<CordonDrainState>) -> Node {
        let status = if online {
            NodeStatus::Online
        } else {
            NodeStatus::Offline
        };
        Node {
            id: id.to_string(),
            status: None,
            spec: Some(NodeSpec::new_all(
                format!("http://{id}:10124"),
                id,
                None::<std::collections::HashMap<String, String>>,
                cordon_drain,
                None::<String>,
                None::<String>,
                None::<bool>,
            )),
            meta: None,
            state: Some(NodeState::new_all(
                format!("http://{id}:10124"),
                id,
                status,
                None::<String>,
                None::<String>,
            )),
        }
    }

    /// Helper to extract metric value from collected metrics.
    fn get_metric_value(metrics: &[proto::MetricFamily], name: &str, node_id: &str) -> Option<f64> {
        for mf in metrics {
            if mf.get_name() == name {
                for metric in mf.get_metric() {
                    for label in metric.get_label() {
                        if label.get_name() == "node_id" && label.get_value() == node_id {
                            return Some(metric.get_gauge().get_value());
                        }
                    }
                }
            }
        }
        None
    }

    #[test]
    fn test_collector_creation() {
        let collector = NodeStatusCollector::new(None);
        // 3 metrics: online, cordoned, draining.
        assert_eq!(collector.desc().len(), 3);
    }

    #[test]
    fn test_collector_no_node() {
        let collector = NodeStatusCollector::new(None);
        let metrics = collector.collect();
        // 3 metric families returned, but with no data points.
        assert_eq!(metrics.len(), 3);
        for mf in &metrics {
            assert_eq!(mf.get_metric().len(), 0);
        }
    }

    #[test]
    fn test_collector_online_node() {
        let node = create_test_node("io-engine-1", true, None);
        let collector = NodeStatusCollector::new(Some(node));

        let metrics = collector.collect();

        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_online", "io-engine-1"),
            Some(1.0)
        );
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_cordoned", "io-engine-1"),
            Some(0.0)
        );
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_draining", "io-engine-1"),
            Some(0.0)
        );
    }

    #[test]
    fn test_collector_offline_node() {
        let node = create_test_node("io-engine-1", false, None);
        let collector = NodeStatusCollector::new(Some(node));

        let metrics = collector.collect();

        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_online", "io-engine-1"),
            Some(0.0)
        );
    }

    #[test]
    fn test_collector_cordoned_node() {
        let cordon =
            CordonDrainState::cordonedstate(CordonedState::new(vec!["maintenance".to_string()]));
        let node = create_test_node("io-engine-1", true, Some(cordon));
        let collector = NodeStatusCollector::new(Some(node));

        let metrics = collector.collect();

        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_cordoned", "io-engine-1"),
            Some(1.0)
        );
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_online", "io-engine-1"),
            Some(1.0)
        );
    }

    #[test]
    fn test_collector_draining_node() {
        let draining = CordonDrainState::drainingstate(DrainState::new(
            vec!["maintenance".to_string()],
            vec!["drain-volumes".to_string()],
        ));
        let node = create_test_node("io-engine-1", true, Some(draining));
        let collector = NodeStatusCollector::new(Some(node));

        let metrics = collector.collect();

        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_draining", "io-engine-1"),
            Some(1.0)
        );
        // Draining implies cordoned.
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_cordoned", "io-engine-1"),
            Some(1.0)
        );
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_online", "io-engine-1"),
            Some(1.0)
        );
    }

    #[test]
    fn test_collector_state_transitions() {
        // Online -> Cordoned -> Draining -> Drained -> Online.
        let node = create_test_node("io-engine-1", true, None);
        let metrics = NodeStatusCollector::new(Some(node)).collect();
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_online", "io-engine-1"),
            Some(1.0)
        );
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_cordoned", "io-engine-1"),
            Some(0.0)
        );

        // Cordoned.
        let cordon =
            CordonDrainState::cordonedstate(CordonedState::new(vec!["maintenance".to_string()]));
        let node = create_test_node("io-engine-1", true, Some(cordon));
        let metrics = NodeStatusCollector::new(Some(node)).collect();
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_cordoned", "io-engine-1"),
            Some(1.0)
        );

        // Draining.
        let draining = CordonDrainState::drainingstate(DrainState::new(
            vec!["maintenance".to_string()],
            vec!["drain-volumes".to_string()],
        ));
        let node = create_test_node("io-engine-1", true, Some(draining));
        let metrics = NodeStatusCollector::new(Some(node)).collect();
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_draining", "io-engine-1"),
            Some(1.0)
        );
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_cordoned", "io-engine-1"),
            Some(1.0)
        );

        // Drained.
        let drained = CordonDrainState::drainedstate(DrainState::new(
            vec!["maintenance".to_string()],
            vec!["drain-volumes".to_string()],
        ));
        let node = create_test_node("io-engine-1", true, Some(drained));
        let metrics = NodeStatusCollector::new(Some(node)).collect();
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_draining", "io-engine-1"),
            Some(1.0)
        );
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_cordoned", "io-engine-1"),
            Some(1.0)
        );

        // Back to online.
        let node = create_test_node("io-engine-1", true, None);
        let metrics = NodeStatusCollector::new(Some(node)).collect();
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_online", "io-engine-1"),
            Some(1.0)
        );
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_cordoned", "io-engine-1"),
            Some(0.0)
        );
        assert_eq!(
            get_metric_value(&metrics, "mayastor_node_draining", "io-engine-1"),
            Some(0.0)
        );
    }

    #[test]
    fn test_collector_metric_names() {
        let node = create_test_node("test-node", true, None);
        let collector = NodeStatusCollector::new(Some(node));

        let metrics = collector.collect();
        let metric_names: Vec<&str> = metrics.iter().map(|m| m.get_name()).collect();

        assert!(metric_names.contains(&"mayastor_node_online"));
        assert!(metric_names.contains(&"mayastor_node_cordoned"));
        assert!(metric_names.contains(&"mayastor_node_draining"));
    }
}
