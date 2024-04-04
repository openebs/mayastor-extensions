use crate::common::{
    constants::PRODUCT,
    error::{
        EventChannelSend, EventPublish, EventRecorderOptionsAbsent, GetPod, JobPodHasTooManyOwners,
        JobPodOwnerIsNotJob, JobPodOwnerNotFound, Result, SerializeEventNote,
    },
    kube_client as KubeClient,
};
use k8s_openapi::{api::core::v1::ObjectReference, serde_json};
use kube::runtime::events::{Event, EventType, Recorder};
use serde::Serialize;
use snafu::{ensure, ResultExt};
use std::{fmt::Display, time::Duration};
use tokio::{select, sync::mpsc, time::sleep};
use tracing::error;

#[derive(Serialize, Debug)]
#[serde(rename_all(serialize = "camelCase"))]
pub(crate) struct EventNote {
    from_version: String,
    to_version: String,
    message: String,
}

impl From<&EventRecorder> for EventNote {
    fn from(er: &EventRecorder) -> EventNote {
        EventNote {
            from_version: er.source_version.clone(),
            to_version: er.target_version.clone(),
            message: Default::default(),
        }
    }
}

impl EventNote {
    fn with_message(mut self, msg: String) -> EventNote {
        self.message = msg;
        self
    }
}

/// A builder for the Kubernetes event publisher.
#[derive(Default)]
pub(crate) struct EventRecorderBuilder {
    pod_name: Option<String>,
    namespace: Option<String>,
    source_version: Option<String>,
    target_version: Option<String>,
}

impl EventRecorderBuilder {
    /// This is a builder option to set the namespace of the object
    /// which will become the 'involvedObject' for the Event.
    #[must_use]
    pub(crate) fn with_namespace<T>(mut self, namespace: T) -> Self
    where
        T: ToString,
    {
        self.namespace = Some(namespace.to_string());
        self
    }

    /// This is a builder option to add the name of this Pod. The owner Job of this Pod
    /// will be the object whose events the publisher will create.
    #[must_use]
    pub(crate) fn with_pod_name<T>(mut self, pod_name: T) -> Self
    where
        T: ToString,
    {
        self.pod_name = Some(pod_name.to_string());
        self
    }

    // TODO: Make the builder option validations error out at compile-time, using std::compile_error
    // or something similar.
    /// This builds the EventRecorder. This fails if Kubernetes API requests fail.
    pub(crate) async fn build(&self) -> Result<EventRecorder> {
        ensure!(
            self.pod_name.is_some() && self.namespace.is_some(),
            EventRecorderOptionsAbsent
        );
        let pod_name = self.pod_name.clone().unwrap();
        let namespace = self.namespace.clone().unwrap();

        // Initialize version to '--'. These can be updated later with set_source_version()
        // and set_target_version() EventRecorder methods.
        let vers_placeholder = "--".to_string();
        let source_version = self
            .source_version
            .clone()
            .unwrap_or(vers_placeholder.clone());
        let target_version = self.target_version.clone().unwrap_or(vers_placeholder);

        let pods_api = KubeClient::pods_api(namespace.as_str()).await?;

        let pod = pods_api.get(pod_name.as_str()).await.context(GetPod {
            pod_name: pod_name.clone(),
            pod_namespace: namespace.clone(),
        })?;

        let pod_owner = match pod.metadata.owner_references {
            Some(references) if references.len() == 1 && references[0].kind.eq("Job") => {
                Ok(references[0].clone())
            }
            Some(references) if references.len() == 1 => JobPodOwnerIsNotJob {
                pod_name: pod_name.clone(),
                pod_namespace: namespace.clone(),
            }
            .fail(),
            Some(references) if references.is_empty() => JobPodOwnerNotFound {
                pod_name: pod_name.clone(),
                pod_namespace: namespace.clone(),
            }
            .fail(),
            Some(_) => JobPodHasTooManyOwners {
                pod_name: pod_name.clone(),
                pod_namespace: namespace.clone(),
            }
            .fail(),
            None => JobPodOwnerNotFound {
                pod_name,
                pod_namespace: namespace.clone(),
            }
            .fail(),
        }?;

        let job_obj_ref = ObjectReference {
            api_version: Some(pod_owner.api_version),
            kind: Some(pod_owner.kind),
            name: Some(pod_owner.name.clone()),
            namespace: Some(namespace),
            uid: Some(pod_owner.uid),
            field_path: None,
            resource_version: None,
        };

        let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
        let k8s_client = KubeClient::client().await?;
        let event_loop_handle = tokio::spawn(async move {
            let event_handler = Recorder::new(k8s_client, pod_owner.name.into(), job_obj_ref);

            // Function exits when 'None'. Avoids panics.
            let mut current_event = rx.recv().await;

            while let Some(event) = &current_event {
                // Hacky clone for the Event.
                let event = Event {
                    type_: event.type_,
                    reason: event.reason.clone(),
                    note: event.note.clone(),
                    action: event.action.clone(),
                    secondary: event.secondary.clone(),
                };
                if let Err(error) = event_handler.publish(event).await.context(EventPublish) {
                    error!(%error);
                }

                select! {
                    _ = sleep(Duration::from_secs(1200)) => {}
                    event = rx.recv() => { current_event = event }
                }
            }
        });

        Ok(EventRecorder {
            event_sender: Some(tx),
            event_loop_handle,
            source_version,
            target_version,
        })
    }
}

/// This is a wrapper around a kube::runtime::events::Recorder.
pub(crate) struct EventRecorder {
    event_sender: Option<mpsc::UnboundedSender<Event>>,
    event_loop_handle: tokio::task::JoinHandle<()>,
    source_version: String,
    target_version: String,
}

impl EventRecorder {
    /// Creates an empty builder.
    pub(crate) fn builder() -> EventRecorderBuilder {
        EventRecorderBuilder::default()
    }

    /// This function is a wrapper around kube::runtime::events' recorder.publish().
    async fn publish(&self, event: Event) -> Result<()> {
        if let Some(sender) = self.event_sender.clone() {
            sender.send(event).map_err(|_| EventChannelSend.build())?;
        }

        Ok(())
    }

    /// This is a helper method with calls the publish method above and fills out the boilerplate
    /// Event fields. type is set to publish a Normal event.
    pub(crate) async fn publish_normal<J, K>(&self, note: J, action: K) -> Result<()>
    where
        J: ToString,
        K: ToString,
    {
        let note = EventNote::from(self).with_message(note.to_string());
        let note_s = serde_json::to_string(&note).context(SerializeEventNote { note })?;
        self.publish(Event {
            type_: EventType::Normal,
            reason: format!("{PRODUCT}Upgrade"),
            note: Some(note_s),
            action: action.to_string(),
            secondary: None,
        })
        .await
    }

    /// This is a helper method with calls the publish method above and fills out the boilerplate
    /// Event fields. type is set to publish a Warning event.
    pub(crate) async fn publish_warning<J, K>(&self, note: J, action: K) -> Result<()>
    where
        J: ToString,
        K: ToString,
    {
        let note = EventNote::from(self).with_message(note.to_string());
        let note_s = serde_json::to_string(&note).context(SerializeEventNote { note })?;
        self.publish(Event {
            type_: EventType::Warning,
            reason: format!("{PRODUCT}Upgrade"),
            note: Some(note_s),
            action: action.to_string(),
            secondary: None,
        })
        .await
    }

    /// This method is intended for use when upgrade fails.
    pub(crate) async fn publish_unrecoverable<Error>(&self, err: &Error, validation_error: bool)
    where
        Error: Display,
    {
        let action = if validation_error {
            EventAction::ValidationFailed
        } else {
            EventAction::Failed
        };
        let _ = self
            .publish_warning(format!("Failed to upgrade: {err}"), action)
            .await
            .map_err(|error| error!(%error, "Failed to upgrade {PRODUCT}"));
    }

    /// Shuts down the event channel which makes the event loop worker exit its loop and return.
    pub(crate) async fn shutdown_worker(mut self) {
        // Dropping the sender, to signify no more channel messages.
        let _ = self.event_sender.take();

        // Wait for event loop to publish its last events and exit.
        let _ = self.event_loop_handle.await;
    }

    /// Updates the EventRecorder's source_version memeber with a new value.
    pub(crate) fn set_source_version(&mut self, version: String) {
        self.source_version = version
    }

    /// Updates the EventRecorder's target_version memeber with a new value.
    pub(crate) fn set_target_version(&mut self, version: String) {
        self.target_version = version
    }
}

/// current volume status
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
pub enum EventAction {
    #[serde(rename = "Failed")]
    Failed,
    #[serde(rename = "Validation Failed")]
    ValidationFailed,
    #[serde(rename = "Upgrading control-plane")]
    UpgradingCP,
    #[serde(rename = "Upgraded control-plane")]
    UpgradedCP,
    #[serde(rename = "Upgrading data-plane")]
    UpgradingDP,
    #[serde(rename = "Upgraded data-plane")]
    UpgradedDP,
    #[serde(rename = "Successful")]
    Successful,
}

impl ToString for EventAction {
    fn to_string(&self) -> String {
        match self {
            Self::Failed => String::from("Failed"),
            Self::ValidationFailed => String::from("Validation Failed"),
            Self::UpgradingCP => String::from("Upgrading control-plane"),
            Self::UpgradedCP => String::from("Upgraded control-plane"),
            Self::UpgradingDP => String::from("Upgrading data-plane"),
            Self::UpgradedDP => String::from("Upgraded data-plane"),
            Self::Successful => String::from("Successful"),
        }
    }
}
