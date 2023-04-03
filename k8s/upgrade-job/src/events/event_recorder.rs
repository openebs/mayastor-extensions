use crate::common::{
    constants::PRODUCT,
    error::{
        EventPublish, EventRecorderOptionsAbsent, GetPod, SerializeEventNote,
        JobPodHasTooManyOwners, JobPodOwnerIsNotJob, JobPodOwnerNotFound, Result,
    },
    kube_client::KubeClientSet,
};
use k8s_openapi::api::core::v1::ObjectReference;
use kube::runtime::events::{Event, EventType, Recorder};
use snafu::{ensure, ResultExt};
use std::fmt::Display;
use k8s_openapi::serde_json;
use serde::Serialize;

#[derive(Serialize, Debug)]
#[serde(rename(serialize = "camelCase"))]
pub(crate) struct EventNote {
    from_version: String,
    to_version: String,
    message: String,
}

impl From<&EventRecorder> for EventNote {
    fn from(er: &EventRecorder) -> EventNote {
        EventNote {
            from_version: er.from_version.clone(),
            to_version: er.to_version.clone(),
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
    reporter_name: Option<String>,
    from_version: Option<String>,
    to_version: Option<String>,
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

    /// This is a builder option to build add the reporter controller name.
    #[must_use]
    pub(crate) fn with_reporter_name(mut self, reporter_name: String) -> Self {
        self.reporter_name = Some(reporter_name);
        self
    }

    /// This is a builder option add the from-version in upgrade.
    #[must_use]
    pub(crate) fn with_from_version(mut self, from: String) -> Self {
        self.from_version = Some(from);
        self
    }

    /// This is a builder option add the to-version in upgrade.
    #[must_use]
    pub(crate) fn with_to_version(mut self, from: String) -> Self {
        self.from_version = Some(from);
        self
    }

    // TODO: Make the builder option validations error out at compile-time, using std::compile_error
    // or something similar.
    /// This builds the EventRecorder. This fails if Kubernetes API requests fail.
    pub(crate) async fn build(&self) -> Result<EventRecorder> {
        ensure!(
            self.reporter_name.is_some() && self.pod_name.is_some() && self.namespace.is_some() && self.from_version.is_some() && self.to_version.is_some(),
            EventRecorderOptionsAbsent
        );
        let reporter_name = self.reporter_name.clone().unwrap();
        let pod_name = self.pod_name.clone().unwrap();
        let namespace = self.namespace.clone().unwrap();
        let from_version = self.from_version.clone().unwrap();
        let to_version = self.to_version.clone().unwrap();

        let k8s_client = KubeClientSet::builder()
            .with_namespace(namespace.as_str())
            .build()
            .await?;

        let pod = k8s_client
            .pods_api()
            .get(pod_name.as_str())
            .await
            .context(GetPod {
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
            name: Some(pod_owner.name),
            namespace: Some(namespace),
            uid: Some(pod_owner.uid),
            field_path: None,
            resource_version: None,
        };

        Ok(EventRecorder {
            k8s_event_recorder: Recorder::new(
                k8s_client.client(),
                reporter_name.into(),
                job_obj_ref,
            ),
            from_version,
            to_version,
        })
    }
}

/// This is a wrapper around a kube::runtime::events::Recorder.
pub(crate) struct EventRecorder {
    k8s_event_recorder: Recorder,
    from_version: String,
    to_version: String,
}

impl EventRecorder {
    /// Creates an empty builder.
    pub(crate) fn builder() -> EventRecorderBuilder {
        EventRecorderBuilder::default()
    }
    /// This function is a wrapper around kube::runtime::events' recorder.publish().
    pub(crate) async fn publish(&self, event: Event) -> Result<()> {
        self.k8s_event_recorder
            .publish(event)
            .await
            .context(EventPublish)
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
    pub(crate) async fn publish_unrecoverable<Error>(&self, err: &Error)
    where
        Error: Display,
    {
        let _ = self
            .publish_warning(format!("Failed to upgrade: {err}"), "Failed")
            .await
            .map_err(|error| tracing::error!(%error, "Failed to upgrade {PRODUCT}"));
    }

}
