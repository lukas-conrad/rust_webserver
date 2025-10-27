pub trait ParsableVariable {
    fn parse(string: String) -> () where Self: Sized;
    fn to_string(&self) -> String;
}
