use crate::managers::transcription::TranscriptionManager;
use crate::transcription_job::{TranscriptionJob, TranscriptionJobQueue, TranscriptionStatus};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub struct TranscriptionJobManager {
    pub job_queue: Arc<Mutex<TranscriptionJobQueue>>,
    app_handle: AppHandle,
    transcription_manager: Arc<TranscriptionManager>,
}

impl TranscriptionJobManager {
    pub fn new(
        app_handle: &AppHandle,
        transcription_manager: Arc<TranscriptionManager>,
    ) -> Self {
        let app_data_dir = app_handle.path().app_data_dir().unwrap();
        let job_queue_path = app_data_dir.join("transcription_queue.json");
        let job_queue = Arc::new(Mutex::new(TranscriptionJobQueue::new(job_queue_path)));

        let manager = Self {
            job_queue,
            app_handle: app_handle.clone(),
            transcription_manager,
        };

        manager.start_worker();
        manager
    }

    fn start_worker(&self) {
        let job_queue = self.job_queue.clone();
        let app_handle = self.app_handle.clone();
        let transcription_manager = self.transcription_manager.clone();

        thread::spawn(move || loop {
            let mut job_id = None;
            let mut file_path = None;

            {
                let mut queue = job_queue.lock().unwrap();
                if !queue.jobs.iter().any(|j| j.status == TranscriptionStatus::Processing) {
                    if let Some(job) = queue.get_next_job() {
                        job_id = Some(job.id.clone());
                        file_path = Some(job.file_path.clone());
                        queue.update_job_status(&job.id, TranscriptionStatus::Processing);
                        app_handle.emit("job-queue-updated", ()).unwrap();
                    }
                }
            }

            if let (Some(id), Some(path)) = (job_id, file_path) {
                let result = transcription_manager.transcribe_file(&id, &path);
                let mut queue = job_queue.lock().unwrap();
                match result {
                    Ok(_) => {
                        queue.update_job_status(&id, TranscriptionStatus::Completed);
                    }
                    Err(e) => {
                        queue.update_job_status(&id, TranscriptionStatus::Failed(e.to_string()));
                    }
                }
                app_handle.emit("job-queue-updated", ()).unwrap();
            }

            thread::sleep(Duration::from_secs(5));
        });
    }
}
