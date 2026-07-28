mod account_controller;
mod capability_controller;
mod channel_config;
mod credential_store;
mod dto;
mod login_controller;
mod login_coordinator;
mod module;
mod monitor;
mod remote_controller;
mod remote_handler;
mod runtime_store;
mod service;

pub(super) use module::WeixinModule;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod credential_store_tests;

#[cfg(test)]
mod login_coordinator_tests;

#[cfg(test)]
mod runtime_store_tests;

#[cfg(test)]
mod monitor_tests;
