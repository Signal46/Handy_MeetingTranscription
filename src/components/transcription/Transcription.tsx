import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';

interface TranscriptionJob {
    id: string;
    file_path: string;
    status: 'Queued' | 'Processing' | 'Completed' | { Failed: string };
    progress: number;
}

const Transcription = () => {
    const [mode, setMode] = useState('dictation');
    const [jobs, setJobs] = useState<TranscriptionJob[]>([]);
    const [isRecording, setIsRecording] = useState(false);

    useEffect(() => {
        if (mode === 'meeting') {
            fetchJobs();
            const unlistenQueue = listen('job-queue-updated', () => {
                fetchJobs();
            });
            const unlistenProgress = listen('transcription-progress', (event) => {
                const { job_id, progress } = event.payload as { job_id: string; progress: number };
                setJobs((prevJobs) =>
                    prevJobs.map((job) =>
                        job.id === job_id ? { ...job, progress } : job
                    )
                );
            });
            return () => {
                unlistenQueue.then(f => f());
                unlistenProgress.then(f => f());
            };
        }
    }, [mode]);

    const fetchJobs = async () => {
        const result: TranscriptionJob[] = await invoke('get_job_queue');
        setJobs(result);
    };

    const handleFileUpload = async () => {
        const result = await open({
            multiple: false,
            filters: [{ name: 'Audio', extensions: ['mp3', 'wav', 'm4a'] }],
        });

        if (result) {
            await invoke('import_audio_file', { path: result.path });
            fetchJobs();
        }
    };

    const handleCancelJob = async (id: string) => {
        await invoke('cancel_transcription_job', { id });
        fetchJobs();
    };

    const handleStartRecording = async () => {
        await invoke('start_file_recording');
        setIsRecording(true);
    };

    const handleStopRecording = async () => {
        await invoke('stop_file_recording');
        setIsRecording(false);
        fetchJobs();
    };

    return (
        <div className="p-4">
            <div className="flex items-center mb-4">
                <label className="mr-2">Mode:</label>
                <select value={mode} onChange={(e) => setMode(e.target.value)} className="p-2 border rounded">
                    <option value="dictation">Dictation</option>
                    <option value="meeting">Meeting</option>
                </select>
            </div>

            {mode === 'meeting' && (
                <div>
                    <div className="mb-4 flex space-x-2">
                        <button onClick={handleFileUpload} className="p-2 bg-blue-500 text-white rounded">
                            Upload Audio File
                        </button>
                        {!isRecording ? (
                            <button onClick={handleStartRecording} className="p-2 bg-green-500 text-white rounded">
                                Start Recording
                            </button>
                        ) : (
                            <button onClick={handleStopRecording} className="p-2 bg-red-500 text-white rounded">
                                Stop Recording
                            </button>
                        )}
                    </div>
                    <div>
                        <h2 className="text-lg font-bold mb-2">Job List</h2>
                        <ul>
                            {jobs.map((job) => (
                                <li key={job.id} className="mb-2 p-2 border rounded">
                                    <div className="flex justify-between items-center">
                                        <span>{job.file_path}</span>
                                        <div>
                                            <span>{typeof job.status === 'object' ? `Failed: ${job.status.Failed}` : job.status}</span>
                                            {job.status === 'Processing' && (
                                                <span className="ml-2">{Math.round(job.progress * 100)}%</span>
                                            )}
                                        </div>
                                        <button onClick={() => handleCancelJob(job.id)} className="p-1 bg-red-500 text-white rounded">
                                            Cancel
                                        </button>
                                    </div>
                                </li>
                            ))}
                        </ul>
                    </div>
                </div>
            )}
        </div>
    );
};

export default Transcription;
