pub type CommitCallback = Box<dyn Fn(String)>;

pub struct PreEdit {
    /// The text currently being composed.
    pub text: String,
    /// The position of the cursor within the preedit text.
    pub cursor: usize,
}

pub enum PreEditEvent {
    /// Preedit text has started.
    Begin,
    /// Preedit text has been updated.
    Update(PreEdit),
    /// Preedit text has ended.
    End,
}
pub type PreEditCallback = Box<dyn Fn(PreEditEvent)>;

pub struct Candidate {
    /// The text of the candidate.
    pub candidates: Vec<String>,
    /// The index of the currently selected candidate.
    pub selected: usize,
}

pub enum CandidateEvent {
    /// Candidate list has started.
    Begin,
    /// Candidate list has been updated.
    Update(Candidate),
    /// Candidate list has ended.
    End,
}
pub type CandidateCallback = Box<dyn Fn(CandidateEvent)>;

pub struct InputMethodInfo {
    /// The unique identifier for the input method.
    pub name: String,
    /// The localized name of the input method.
    pub localized_name: String,
    /// The language code associated with the input method.
    pub language: String,
    /// The localized language name associated with the input method.
    pub localized_language: String,
}

pub enum InputMethod {
    /// The input method retrieve is not supported.
    Unsupported,
    Info(InputMethodInfo),
}
pub type InputMethodCallback = Box<dyn Fn(InputMethod)>;

pub enum InputMode {
    /// Input mode retrieve is not supported.
    Unsupported,
    /// Input mode for typing in English.
    Alpha,
    /// Input mode for typing in another language.
    Native,
}
pub type InputModeCallback = Box<dyn Fn(InputMode)>;

pub trait InputContext {
    /// Retrieves active input method
    fn get_input_method(&self) -> InputMethod;

    /// Retrieves the current input mode.
    fn get_input_mode(&self) -> InputMode;

    /// Set if the input method is activated.
    fn get_activated(&self) -> bool;
    /// Get if the input method is activated.
    fn set_activated(&mut self, activated: bool);

    /// Sets the rectangle area for positioning candidate window.
    fn set_preedit_rect(&mut self, x: i32, y: i32, width: i32, height: i32);

    fn set_commit_callback(&mut self, callback: CommitCallback);
    fn set_preedit_callback(&mut self, callback: PreEditCallback);
    fn set_candidate_callback(&mut self, callback: CandidateCallback);
    fn set_input_method_callback(&mut self, callback: InputMethodCallback);
    fn set_input_mode_callback(&mut self, callback: InputModeCallback);
}
