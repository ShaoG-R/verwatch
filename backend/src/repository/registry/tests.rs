use super::super::adapter::tests::MockEnv;
use super::super::adapter::{MonitorClient, RegistryStorageAdapter};
use super::*;
use crate::error::{WatchError, WatchResult};
use async_trait::async_trait;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use verwatch_shared::{BaseConfig, CreateProjectRequest, MonitorState, TimeConfig};

// =========================================================
// Shared Mock Components
// =========================================================

struct TestContext {
    /// Operation log to verify calling order
    log: RefCell<Vec<String>>,
    /// In-memory storage of keys
    storage_keys: RefCell<HashSet<String>>,
    /// In-memory storage of monitor configs
    monitor_configs: RefCell<HashMap<String, ProjectConfig>>,
    /// Set of keys to simulate failure on get_config
    fail_get_config_keys: RefCell<HashSet<String>>,
}

impl TestContext {
    fn new() -> Self {
        Self {
            log: RefCell::new(Vec::new()),
            storage_keys: RefCell::new(HashSet::new()),
            monitor_configs: RefCell::new(HashMap::new()),
            fail_get_config_keys: RefCell::new(HashSet::new()),
        }
    }

    fn push_log(&self, msg: String) {
        self.log.borrow_mut().push(msg);
    }
}

struct TestStorage {
    ctx: Rc<TestContext>,
}

#[async_trait(?Send)]
impl RegistryStorageAdapter for TestStorage {
    async fn add(&self, key: &str) -> WatchResult<()> {
        self.ctx.push_log(format!("storage:add:{}", key));
        self.ctx.storage_keys.borrow_mut().insert(key.to_string());
        Ok(())
    }

    async fn remove(&self, key: &str) -> WatchResult<bool> {
        self.ctx.push_log(format!("storage:remove:{}", key));
        Ok(self.ctx.storage_keys.borrow_mut().remove(key))
    }

    async fn list(&self) -> WatchResult<Vec<String>> {
        self.ctx.push_log("storage:list".to_string());
        Ok(self.ctx.storage_keys.borrow().iter().cloned().collect())
    }

    async fn contains(&self, key: &str) -> WatchResult<bool> {
        self.ctx.push_log(format!("storage:contains:{}", key));
        Ok(self.ctx.storage_keys.borrow().contains(key))
    }
}

struct TestMonitorClient {
    ctx: Rc<TestContext>,
}

#[async_trait(?Send)]
impl MonitorClient for TestMonitorClient {
    async fn setup(&self, unique_key: &str, config: &ProjectConfig) -> WatchResult<()> {
        self.ctx.push_log(format!("monitor:setup:{}", unique_key));
        self.ctx
            .monitor_configs
            .borrow_mut()
            .insert(unique_key.to_string(), config.clone());
        Ok(())
    }

    async fn stop(&self, unique_key: &str) -> WatchResult<()> {
        self.ctx.push_log(format!("monitor:stop:{}", unique_key));
        Ok(())
    }

    async fn get_config(&self, unique_key: &str) -> WatchResult<Option<ProjectConfig>> {
        self.ctx
            .push_log(format!("monitor:get_config:{}", unique_key));
        if self.ctx.fail_get_config_keys.borrow().contains(unique_key) {
            return Err(WatchError::store("Simulated failure"));
        }
        Ok(self.ctx.monitor_configs.borrow().get(unique_key).cloned())
    }

    async fn switch(&self, unique_key: &str, paused: bool) -> WatchResult<()> {
        self.ctx
            .push_log(format!("monitor:switch:{}:{}", unique_key, paused));
        Ok(())
    }

    async fn trigger_check(&self, unique_key: &str) -> WatchResult<()> {
        self.ctx
            .push_log(format!("monitor:trigger_check:{}", unique_key));
        Ok(())
    }
}

// Helper to create logic instance
fn setup_env() -> (
    Rc<TestContext>,
    ProjectRegistryLogic<TestStorage, MockEnv, TestMonitorClient>,
) {
    let ctx = Rc::new(TestContext::new());
    let storage = TestStorage { ctx: ctx.clone() };
    let client = TestMonitorClient { ctx: ctx.clone() };
    let env = MockEnv::new(); // Assuming MockEnv::new() exists
    let logic = ProjectRegistryLogic::new(storage, env, client);
    (ctx, logic)
}

fn make_test_config(key: &str) -> ProjectConfig {
    ProjectConfig {
        unique_key: key.to_string(),
        request: CreateProjectRequest {
            base_config: BaseConfig {
                upstream_owner: "owner".into(),
                upstream_repo: "repo".into(),
                my_owner: "my".into(),
                my_repo: "my-repo".into(),
            },
            time_config: TimeConfig::default(),
            comparison_mode: verwatch_shared::ComparisonMode::PublishedAt,
            dispatch_token_secret: None,
            initial_delay: verwatch_shared::DurationSecs::from_secs(0),
        },
        state: MonitorState::Paused,
    }
}

// =========================================================
// Tests
// =========================================================

#[tokio::test]
async fn test_registry_flow() {
    let (_, logic) = setup_env();

    // 1. Register projects
    logic
        .register(RegisterMonitorCmd {
            config: make_test_config("project-a"),
        })
        .await
        .unwrap();

    logic
        .register(RegisterMonitorCmd {
            config: make_test_config("project-b"),
        })
        .await
        .unwrap();

    // 2. List
    let list = logic.list(ListMonitorsCmd).await.unwrap();
    assert_eq!(list.len(), 2);
    let keys: Vec<_> = list.iter().map(|c| c.unique_key.as_str()).collect();
    assert!(keys.contains(&"project-a"));
    assert!(keys.contains(&"project-b"));

    // 3. Check existence
    assert!(
        logic
            .is_registered(IsRegisteredCmd {
                unique_key: "project-a".into()
            })
            .await
            .unwrap()
    );

    // 4. Unregister
    assert!(
        logic
            .unregister(UnregisterMonitorCmd {
                unique_key: "project-a".into()
            })
            .await
            .unwrap()
    );

    // 5. Verify removal
    let list_after = logic.list(ListMonitorsCmd).await.unwrap();
    assert_eq!(list_after.len(), 1);
    assert_eq!(list_after[0].unique_key, "project-b");
}

#[tokio::test]
async fn test_unregister_nonexistent() {
    let (_, logic) = setup_env();
    let result = logic
        .unregister(UnregisterMonitorCmd {
            unique_key: "nonexistent".into(),
        })
        .await
        .unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_register_duplicate_key() {
    let (_, logic) = setup_env();
    let config = make_test_config("dup-key");

    let key1 = logic
        .register(RegisterMonitorCmd {
            config: config.clone(),
        })
        .await
        .unwrap();
    assert_eq!(key1, "dup-key");

    // Second registration (overwrite)
    let key2 = logic.register(RegisterMonitorCmd { config }).await.unwrap();
    assert_eq!(key2, "dup-key");

    let list = logic.list(ListMonitorsCmd).await.unwrap();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
async fn test_is_registered_nonexistent() {
    let (_, logic) = setup_env();
    assert!(
        !logic
            .is_registered(IsRegisteredCmd {
                unique_key: "nope".into()
            })
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_list_empty() {
    let (_, logic) = setup_env();
    let list = logic.list(ListMonitorsCmd).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn test_setup_called() {
    let (ctx, logic) = setup_env();
    logic
        .register(RegisterMonitorCmd {
            config: make_test_config("setup-test"),
        })
        .await
        .unwrap();

    let logs = ctx.log.borrow();
    assert!(logs.contains(&"monitor:setup:setup-test".to_string()));
    assert!(logs.contains(&"storage:add:setup-test".to_string()));
}

#[tokio::test]
async fn test_stop_called() {
    let (ctx, logic) = setup_env();
    logic
        .register(RegisterMonitorCmd {
            config: make_test_config("stop-test"),
        })
        .await
        .unwrap();

    // Clear logs to focus on unregister
    ctx.log.borrow_mut().clear();

    logic
        .unregister(UnregisterMonitorCmd {
            unique_key: "stop-test".into(),
        })
        .await
        .unwrap();

    let logs = ctx.log.borrow();

    let stop_idx = logs
        .iter()
        .position(|r| r == "monitor:stop:stop-test")
        .expect("should call stop");
    let remove_idx = logs
        .iter()
        .position(|r| r == "storage:remove:stop-test")
        .expect("should call remove");
    let contains_idx = logs
        .iter()
        .position(|r| r == "storage:contains:stop-test")
        .expect("should check contains");

    assert!(
        contains_idx < stop_idx,
        "should check existence before stop"
    );
    assert!(stop_idx < remove_idx, "should stop before remove");
}

#[tokio::test]
async fn test_list_with_partial_failure() {
    let (ctx, logic) = setup_env();

    for key in ["good-1", "bad-1", "good-2"] {
        logic
            .register(RegisterMonitorCmd {
                config: make_test_config(key),
            })
            .await
            .unwrap();
    }

    // Set bad-1 to fail
    ctx.fail_get_config_keys.borrow_mut().insert("bad-1".into());

    let list = logic.list(ListMonitorsCmd).await.unwrap();
    assert_eq!(list.len(), 2);
    let keys: Vec<_> = list.iter().map(|c| c.unique_key.as_str()).collect();
    assert!(keys.contains(&"good-1"));
    assert!(keys.contains(&"good-2"));
    assert!(!keys.contains(&"bad-1"));
}

// New Tests

#[tokio::test]
async fn test_switch_monitor_success() {
    let (ctx, logic) = setup_env();
    logic
        .register(RegisterMonitorCmd {
            config: make_test_config("switch-test"),
        })
        .await
        .unwrap();

    ctx.log.borrow_mut().clear();

    let result = logic
        .switch_monitor(RegistrySwitchMonitorCmd {
            unique_key: "switch-test".into(),
            paused: true,
        })
        .await
        .unwrap();

    assert!(result);
    // Verify monitor switch called
    let logs = ctx.log.borrow();
    assert!(logs.iter().any(|s| s == "monitor:switch:switch-test:true"));
}

#[tokio::test]
async fn test_switch_monitor_not_found() {
    let (ctx, logic) = setup_env();

    let result = logic
        .switch_monitor(RegistrySwitchMonitorCmd {
            unique_key: "not-found".into(),
            paused: true,
        })
        .await
        .unwrap();

    assert!(!result);
    // Verify monitor switch NOT called
    let logs = ctx.log.borrow();
    assert!(!logs.iter().any(|s| s.starts_with("monitor:switch")));
}

#[tokio::test]
async fn test_trigger_check_success() {
    let (ctx, logic) = setup_env();
    logic
        .register(RegisterMonitorCmd {
            config: make_test_config("check-test"),
        })
        .await
        .unwrap();

    ctx.log.borrow_mut().clear();

    let result = logic
        .trigger_check(RegistryTriggerCheckCmd {
            unique_key: "check-test".into(),
        })
        .await
        .unwrap();

    assert!(result);
    // Verify monitor trigger called
    let logs = ctx.log.borrow();
    assert!(logs.iter().any(|s| s == "monitor:trigger_check:check-test"));
}

#[tokio::test]
async fn test_trigger_check_not_found() {
    let (ctx, logic) = setup_env();

    let result = logic
        .trigger_check(RegistryTriggerCheckCmd {
            unique_key: "not-found".into(),
        })
        .await
        .unwrap();

    assert!(!result);
    let logs = ctx.log.borrow();
    assert!(!logs.iter().any(|s| s.starts_with("monitor:trigger_check")));
}
