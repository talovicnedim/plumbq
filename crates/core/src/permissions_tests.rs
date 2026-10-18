use super::*;

fn ask(strong: bool) -> PermissionDecision {
    PermissionDecision::Ask { strong }
}

#[test]
fn default_policy_matrix() {
    let policy = AgentPermissions::default();
    let plain = PermissionContext::default();
    let production = PermissionContext {
        production: true,
        ..Default::default()
    };
    assert_eq!(
        decide(Permission::ReadWorkspace, &plain, &policy),
        PermissionDecision::Allow
    );
    assert_eq!(
        decide(Permission::SendSafe, &plain, &policy),
        PermissionDecision::Allow
    );
    assert_eq!(
        decide(Permission::SendSafe, &production, &policy),
        PermissionDecision::Allow
    );
    assert_eq!(decide(Permission::SendWrite, &plain, &policy), ask(false));
    assert_eq!(
        decide(Permission::SendWrite, &production, &policy),
        ask(true)
    );
    assert_eq!(decide(Permission::EditFiles, &plain, &policy), ask(false));
    assert_eq!(
        decide(Permission::ChangeEnvironment, &plain, &policy),
        ask(false)
    );
    assert_eq!(decide(Permission::Destructive, &plain, &policy), ask(true));
}

#[test]
fn risky_context_asks_even_when_allowed() {
    let policy = AgentPermissions {
        send_write: PermissionRule::Allow,
        change_environment: PermissionRule::Allow,
        ..AgentPermissions::default()
    };
    let new_host = PermissionContext {
        secrets_to_new_host: Some("evil.test".into()),
        ..Default::default()
    };
    assert_eq!(decide(Permission::SendSafe, &new_host, &policy), ask(false));
    let unexpected_host = PermissionContext {
        unexpected_host: Some("a download from 127.0.0.1".into()),
        ..Default::default()
    };
    assert_eq!(
        decide(Permission::SendSafe, &unexpected_host, &policy),
        ask(false)
    );
    let production = PermissionContext {
        production: true,
        ..Default::default()
    };
    assert_eq!(
        decide(Permission::SendWrite, &production, &policy),
        ask(true)
    );
    let repoint = PermissionContext {
        repoints_url: Some("baseUrl to evil.test".into()),
        ..Default::default()
    };
    assert_eq!(
        decide(Permission::ChangeEnvironment, &repoint, &policy),
        ask(true)
    );
}

#[test]
fn production_environment_changes_need_the_typed_name() {
    let policy = AgentPermissions {
        edit_files: PermissionRule::Allow,
        change_environment: PermissionRule::Allow,
        ..AgentPermissions::default()
    };
    let production = PermissionContext {
        production: true,
        ..Default::default()
    };
    for permission in [Permission::EditFiles, Permission::ChangeEnvironment] {
        assert_eq!(
            decide(permission, &production, &policy),
            ask(true),
            "{permission:?}"
        );
    }
    assert_eq!(
        decide(
            Permission::ChangeEnvironment,
            &production,
            &AgentPermissions::default()
        ),
        ask(true)
    );
    assert_eq!(
        decide(Permission::ReadWorkspace, &production, &policy),
        PermissionDecision::Allow
    );
}

#[test]
fn deny_and_read_only_never_ask() {
    let policy = AgentPermissions::read_only();
    let plain = PermissionContext::default();
    for permission in [
        Permission::SendWrite,
        Permission::EditFiles,
        Permission::ChangeEnvironment,
        Permission::Destructive,
    ] {
        assert!(
            matches!(
                decide(permission, &plain, &policy),
                PermissionDecision::Deny { .. }
            ),
            "{permission:?}"
        );
    }
    assert_eq!(
        decide(Permission::SendSafe, &plain, &policy),
        PermissionDecision::Allow
    );
    assert_eq!(
        decide(Permission::ReadWorkspace, &plain, &policy),
        PermissionDecision::Allow
    );
}

#[test]
fn process_env_is_denied_unless_allowed() {
    let env = PermissionContext {
        process_env: true,
        ..Default::default()
    };
    let mut policy = AgentPermissions::default();
    assert!(matches!(
        decide(Permission::SendSafe, &env, &policy),
        PermissionDecision::Deny { .. }
    ));
    policy.allow_process_env = true;
    assert_eq!(
        decide(Permission::SendSafe, &env, &policy),
        PermissionDecision::Allow
    );
}

#[test]
fn forbidden_calls_are_refused_under_any_rules() {
    let allow_all = AgentPermissions {
        send_write: PermissionRule::Allow,
        ..AgentPermissions::default()
    };
    let context = PermissionContext {
        forbidden: Some("the body sends .env".into()),
        ..Default::default()
    };
    assert_eq!(
        decide(Permission::SendSafe, &context, &allow_all),
        PermissionDecision::Deny {
            reason: "the body sends .env".into()
        }
    );
}
