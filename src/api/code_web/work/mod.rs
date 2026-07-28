mod controller;
mod folders;
mod model;
mod module;
mod service;
mod storage;
mod validation;

pub(in crate::api::code_web) use module::WorkModule;

#[cfg(test)]
mod tests;
