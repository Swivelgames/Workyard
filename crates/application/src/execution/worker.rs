pub(crate) async fn blocking_action<F>(operation: F) -> F::Output
where
	F: std::future::Future + Send + 'static,
	F::Output: Send + 'static,
{
	smol::unblock(move || smol::block_on(operation)).await
}
