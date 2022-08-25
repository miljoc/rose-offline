use arrayvec::ArrayVec;
use std::sync::Arc;

use rose_data::{JobClassData, JobClassDatabase, JobId, StringDatabase};
use rose_file_readers::{stb_column, StbFile, VirtualFilesystem};

struct StbJobClass(StbFile);

impl StbJobClass {
    stb_column! { 1..=8, get_jobs, ArrayVec<JobId, 8> }
}

pub fn get_job_class_database(
    vfs: &VirtualFilesystem,
    string_database: Arc<StringDatabase>,
) -> Result<JobClassDatabase, anyhow::Error> {
    let stb = StbJobClass(vfs.read_file::<StbFile, _>("3DDATA/STB/LIST_CLASS.STB")?);
    let mut job_classes = Vec::new();
    for row in 0..stb.0.rows() {
        let jobs = stb.get_jobs(row);
        if jobs.is_empty() {
            job_classes.push(None);
            continue;
        }

        let name = stb
            .0
            .try_get(row, stb.0.columns() - 1)
            .and_then(|key| string_database.get_job_class_name(key));
        job_classes.push(Some(JobClassData {
            name: name.map_or("", |x| unsafe { std::mem::transmute(x) }),
            jobs,
        }));
    }

    Ok(JobClassDatabase::new(string_database, job_classes))
}
