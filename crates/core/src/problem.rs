#[derive(Clone, Debug)]
pub struct Problem {
	pub code: crate::ProblemCode,
	pub message: String,
	pub remedy: String,
}

impl Problem {
	pub fn new(code: crate::ProblemCode, message: impl Into<String>) -> Self {
		Self {
			code,
			message: message.into(),
			remedy: String::new(),
		}
	}

	pub fn remedy(mut self, remedy: impl Into<String>) -> Self {
		self.remedy = remedy.into();
		self
	}

	pub fn io(context: impl std::fmt::Display, error: std::io::Error) -> Self {
		Self::new(crate::ProblemCode::IoError, format!("{context}: {error}"))
	}
}

impl std::fmt::Display for Problem {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}: {}", self.code, self.message)
	}
}
impl std::error::Error for Problem {}
