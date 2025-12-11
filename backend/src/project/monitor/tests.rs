use super::*;
use crate::project::adapter::tests::{MockEnv, MockStorage};
use crate::utils::request::MockHttpClient;
use std::time::Duration;
use verwatch_shared::{BaseConfig, ComparisonMode, CreateProjectRequest, DurationSecs, TimeConfig};

// =========================================================
// 辅助函数
// =========================================================

fn create_test_config() -> ProjectConfig {
    let request = CreateProjectRequest {
        base_config: BaseConfig {
            upstream_owner: "owner".to_string(),
            upstream_repo: "repo".to_string(),
            my_owner: "my_owner".to_string(),
            my_repo: "my_repo".to_string(),
        },
        time_config: TimeConfig::default(),
        initial_delay: DurationSecs::from_secs(60),
        dispatch_token_secret: None,
        comparison_mode: ComparisonMode::PublishedAt,
    };
    ProjectConfig::new(request)
}

fn create_logic(
    storage: MockStorage,
    env: MockEnv,
    client: MockHttpClient,
) -> ProjectMonitorLogicTestable<MockStorage, MockEnv, MockHttpClient> {
    ProjectMonitorLogicTestable::new(storage, env, client)
}

// =========================================================
// setup 测试
// =========================================================

#[tokio::test]
async fn test_setup_saves_config() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    let config = create_test_config();
    let cmd = SetupMonitorCmd {
        config: config.clone(),
    };

    logic.setup(cmd).await.unwrap();

    // 验证 config 已保存
    let saved: Option<ProjectConfig> = logic.storage.get(STATE_KEY_CONFIG).await.unwrap();
    assert!(saved.is_some());
    let saved_config = saved.unwrap();
    assert_eq!(saved_config.unique_key, config.unique_key);
}

#[tokio::test]
async fn test_setup_sets_running_state() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    let config = create_test_config();
    let cmd = SetupMonitorCmd { config };

    logic.setup(cmd).await.unwrap();

    // 验证状态是 Running
    let saved: Option<ProjectConfig> = logic.storage.get(STATE_KEY_CONFIG).await.unwrap();
    let saved_config = saved.unwrap();
    assert!(!saved_config.state.is_paused());
}

#[tokio::test]
async fn test_setup_sets_alarm() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    let delay = DurationSecs::from_secs(120);
    let mut config = create_test_config();
    config.request.initial_delay = delay;

    let cmd = SetupMonitorCmd { config };

    logic.setup(cmd).await.unwrap();

    // 验证 alarm 已设置
    let alarm = logic.storage.alarm.borrow();
    assert_eq!(*alarm, Some(Duration::from(delay)));
}

// =========================================================
// stop 测试
// =========================================================

#[tokio::test]
async fn test_stop_clears_config() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    // 先 setup
    let config = create_test_config();
    logic.setup(SetupMonitorCmd { config }).await.unwrap();

    // 执行 stop
    logic.stop(StopMonitorCmd).await.unwrap();

    // 验证 config 已删除
    let saved: Option<ProjectConfig> = logic.storage.get(STATE_KEY_CONFIG).await.unwrap();
    assert!(saved.is_none());
}

#[tokio::test]
async fn test_stop_clears_version() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    // 先 setup 并手动设置版本
    let config = create_test_config();
    logic.setup(SetupMonitorCmd { config }).await.unwrap();
    logic
        .storage
        .put(STATE_KEY_VERSION, &"v1.0.0".to_string())
        .await
        .unwrap();

    // 执行 stop
    logic.stop(StopMonitorCmd).await.unwrap();

    // 验证 version 已删除
    let version: Option<String> = logic.storage.get(STATE_KEY_VERSION).await.unwrap();
    assert!(version.is_none());
}

#[tokio::test]
async fn test_stop_clears_alarm() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    // 先 setup
    let config = create_test_config();
    logic.setup(SetupMonitorCmd { config }).await.unwrap();

    // 确认有 alarm
    assert!(logic.storage.alarm.borrow().is_some());

    // 执行 stop
    logic.stop(StopMonitorCmd).await.unwrap();

    // 验证 alarm 已删除
    assert!(logic.storage.alarm.borrow().is_none());
}

// =========================================================
// get_config 测试
// =========================================================

#[tokio::test]
async fn test_get_config_returns_none_when_empty() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    let result = logic.get_config(GetConfigCmd).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_config_returns_saved_config() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    let config = create_test_config();
    logic
        .setup(SetupMonitorCmd {
            config: config.clone(),
        })
        .await
        .unwrap();

    let result = logic.get_config(GetConfigCmd).await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().unique_key, config.unique_key);
}

// =========================================================
// switch_monitor 测试
// =========================================================

#[tokio::test]
async fn test_switch_monitor_no_config_returns_error() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    let result = logic
        .switch_monitor(SwitchMonitorCmd { paused: true })
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_switch_monitor_pause() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    // 先 setup (状态为 Running)
    let config = create_test_config();
    logic.setup(SetupMonitorCmd { config }).await.unwrap();

    // 切换到暂停
    logic
        .switch_monitor(SwitchMonitorCmd { paused: true })
        .await
        .unwrap();

    // 验证状态为 Paused
    let saved: Option<ProjectConfig> = logic.storage.get(STATE_KEY_CONFIG).await.unwrap();
    assert!(saved.unwrap().state.is_paused());

    // 验证 alarm 已删除
    assert!(logic.storage.alarm.borrow().is_none());
}

#[tokio::test]
async fn test_switch_monitor_resume() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    // 先 setup
    let config = create_test_config();
    logic.setup(SetupMonitorCmd { config }).await.unwrap();

    // 暂停
    logic
        .switch_monitor(SwitchMonitorCmd { paused: true })
        .await
        .unwrap();

    // 恢复
    logic
        .switch_monitor(SwitchMonitorCmd { paused: false })
        .await
        .unwrap();

    // 验证状态为 Running
    let saved: Option<ProjectConfig> = logic.storage.get(STATE_KEY_CONFIG).await.unwrap();
    assert!(!saved.unwrap().state.is_paused());

    // 验证 alarm 已设置 (立即触发，所以是 0)
    assert_eq!(
        *logic.storage.alarm.borrow(),
        Some(Duration::from_millis(0))
    );
}

#[tokio::test]
async fn test_switch_monitor_same_state_noop() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    // 先 setup (状态为 Running)
    let config = create_test_config();
    logic.setup(SetupMonitorCmd { config }).await.unwrap();
    let alarm_before = *logic.storage.alarm.borrow();

    // 切换到 Running (已经是 Running)
    logic
        .switch_monitor(SwitchMonitorCmd { paused: false })
        .await
        .unwrap();

    // 状态不变，alarm 也不变
    let alarm_after = *logic.storage.alarm.borrow();
    assert_eq!(alarm_before, alarm_after);
}

// =========================================================
// trigger 测试 (无 config 情况)
// =========================================================

#[tokio::test]
async fn test_trigger_no_config_returns_error() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    let result = logic.trigger(TriggerCheckCmd).await;
    assert!(result.is_err());
}

// =========================================================
// on_alarm 测试
// =========================================================

#[tokio::test]
async fn test_on_alarm_no_config_deletes_alarm() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();

    // 手动设置 alarm 但没有 config
    *storage.alarm.borrow_mut() = Some(Duration::from_secs(60));

    let logic = create_logic(storage, env, client);
    logic.on_alarm().await.unwrap();

    // alarm 应该被删除
    assert!(logic.storage.alarm.borrow().is_none());
}

#[tokio::test]
async fn test_on_alarm_paused_deletes_alarm() {
    let storage = MockStorage::new();
    let env = MockEnv::new();
    let client = MockHttpClient::new();
    let logic = create_logic(storage, env, client);

    // setup 然后暂停
    let config = create_test_config();
    logic.setup(SetupMonitorCmd { config }).await.unwrap();
    logic
        .switch_monitor(SwitchMonitorCmd { paused: true })
        .await
        .unwrap();

    // 手动设置 alarm 模拟意外情况
    *logic.storage.alarm.borrow_mut() = Some(Duration::from_secs(60));

    // 触发 alarm
    logic.on_alarm().await.unwrap();

    // alarm 应该被删除
    assert!(logic.storage.alarm.borrow().is_none());
}

// =========================================================
// MockEnv 测试
// =========================================================

#[test]
fn test_mock_env_var() {
    let env = MockEnv::new()
        .with_var("TEST_VAR", "test_value")
        .with_var("ANOTHER_VAR", "another_value");

    assert_eq!(env.var("TEST_VAR"), Some("test_value".to_string()));
    assert_eq!(env.var("ANOTHER_VAR"), Some("another_value".to_string()));
    assert_eq!(env.var("NONEXISTENT"), None);
}

#[test]
fn test_mock_env_secret() {
    let env = MockEnv::new()
        .with_secret("API_KEY", "secret123")
        .with_secret("TOKEN", "token456");

    assert_eq!(env.secret("API_KEY"), Some("secret123".to_string()));
    assert_eq!(env.secret("TOKEN"), Some("token456".to_string()));
    assert_eq!(env.secret("NONEXISTENT"), None);
}
