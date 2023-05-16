/// See [`spawn_attached`]
#[must_use = "dropping this type aborts the task"]
pub struct AttachedTask(tokio::task::JoinHandle<()>);
impl Drop for AttachedTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
/// Wrapper around [`tokio::spawn`] that aborts the task instead of detaching when dropped
///
/// Useful for utility tasks that shouldn't outlive their "parent" task
pub fn spawn_attached(f: impl std::future::Future<Output = ()> + Send + 'static) -> AttachedTask {
    AttachedTask(tokio::spawn(f))
}
