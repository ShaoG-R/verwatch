//! 路由定义模块 - 领域模型
//!
//! 这是纯粹的业务逻辑层，不依赖于 DOM 或 web_sys。
//! 定义了应用的所有路由及其属性。

use std::fmt::Display;

/// 应用路由枚举
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AppRoute {
    /// 登录页面 (默认路由)
    #[default]
    Login,
    /// 控制面板 (需要认证)
    Dashboard,
    /// 页面未找到
    NotFound,
}

impl AppRoute {
    /// 将 URL path 解析为路由枚举
    pub fn from_path(path: &str) -> Self {
        match path {
            "/" | "/login" => Self::Login,
            "/dashboard" => Self::Dashboard,
            _ => Self::NotFound,
        }
    }

    /// 获取路由对应的 URL path
    pub fn to_path(&self) -> &'static str {
        match self {
            Self::Login => "/",
            Self::Dashboard => "/dashboard",
            Self::NotFound => "/404",
        }
    }

    /// **核心守卫逻辑：定义该路由是否需要认证**
    pub fn requires_auth(&self) -> bool {
        matches!(self, Self::Dashboard)
    }

    /// 定义已认证用户是否应该离开此路由（如登录页）
    pub fn should_redirect_when_authenticated(&self) -> bool {
        matches!(self, Self::Login)
    }

    /// 获取认证失败时的重定向目标
    pub fn auth_failure_redirect() -> Self {
        Self::Login
    }

    /// 获取认证成功时的重定向目标（从登录页）
    pub fn auth_success_redirect() -> Self {
        Self::Dashboard
    }
}

impl Display for AppRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_path())
    }
}
