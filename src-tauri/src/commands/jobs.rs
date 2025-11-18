use crate::managers::job::TranscriptionJobManager;
use crate::managers::meeting_recorder::MeetingRecorder;
use crate::transcription_job::{normalize_audio, TranscriptionJob, TranscriptionStatus};
use std::sync::{Mutex, Arc};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;
use std::path::Path;

#[tauri::command]
pub fn start_file_recording(
    recorder_state: State<'_, Arc<Mutex<MeetingRecorder>>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let mut recorder = recorder_state.lock().unwrap();
    let output_dir = app_handle.path().app_data_dir().unwrap();
    recorder
        .start_recording(&output_dir)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_file_recording(
    recorder_state: State<'_, Arc<Mutex<MeetingRecorder>>>,
    job_manager: State<'_, TranscriptionJobManager>,
) -> Result<(), String> {
    let mut recorder = recorder_state.lock().unwrap();
    let file_path = recorder.stop_recording().map_err(|e| e.to_string())?;
    let job = TranscriptionJob {
        id: Uuid::new_v4().to_string(),
        file_path,
        status: TranscriptionStatus::Queued,
        progress: 0.0,
    };
    job_manager.job_queue.lock().unwrap().add_job(job);
    Ok(())
}

#[tauri::command]
pub async fn import_audio_file(
    path: String,
    job_manager: State<'_, TranscriptionJobManager>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let temp_dir = app_handle.path().app_temp_dir().unwrap();
    let normalized_path = normalize_audio(Path::new(&path), &temp_dir).map_err(|e| e.to_string())?;
    let job = TranscriptionJob {
        id: Uuid::new_v4().to_string(),
        file_path: normalized_path,
        status: TranscriptionStatus::Queued,
        progress: 0.0,
    };
    job_manager.job_queue.lock().unwrap().add_job(job);
    Ok(())
}


#[tauri::command]
pub fn get_job_queue(job_manager: State<'_, TranscriptionJobManager>) -> Result<Vec<TranscriptionJob>, String> {
    Ok(job_manager.job_queue.lock().unwrap().jobs.clone())
}

#[tauri::command]
pub fn cancel_transcription_job(
    id: String,
    job_manager: State<'_, TranscriptionJobManager>,
) -> Result<bool, String> {
    Ok(job_manager.job_queue.lock().unwrap().cancel_job(&id))
}
