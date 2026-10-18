use super::*;
use crate::workspace::tests::{temp_workspace, write};

fn no_env(_: &str) -> Option<String> {
    None
}

#[test]
fn parses_dynamic_names() {
    assert_eq!(parse("$prompt otp"), Some(Source::Prompt("otp")));
    assert_eq!(parse("$processEnv HOME"), Some(Source::ProcessEnv("HOME")));
    assert_eq!(parse("$env.HOME"), Some(Source::ProcessEnv("HOME")));
    assert_eq!(parse("$dotenv API_KEY"), Some(Source::Dotenv("API_KEY")));
    assert_eq!(parse("baseUrl"), None);
    assert_eq!(parse("$uuid"), None);
}

#[test]
fn lists_prompts_once() {
    let text = "{{$prompt otp}} {{token}} {{ $prompt otp }} {{$prompt code}}";
    assert_eq!(prompts(text), ["otp", "code"]);
}

#[test]
fn missing_prompt_answers_need_input() {
    let mut values = Values::default();
    let result = resolve(
        "{{$prompt otp}}",
        Path::new("/nowhere"),
        &BTreeMap::new(),
        &no_env,
        &mut values,
    );
    assert!(matches!(result, Err(Error::NeedsInput(names)) if names == ["otp"]));
}

#[test]
fn prompt_answers_substitute_and_are_masked() {
    let text = "code={{$prompt otp}}";
    let answers = BTreeMap::from([("otp".to_string(), "123456".to_string())]);
    let mut values = Values::default();
    resolve(text, Path::new("/nowhere"), &answers, &no_env, &mut values).unwrap();

    assert_eq!(variables::substitute(text, &values).unwrap(), "code=123456");
    assert_eq!(
        variables::mask("sent 123456", &values.secrets),
        "sent {{$prompt otp}}"
    );
}

#[test]
fn process_env_masks_only_secret_looking_names() {
    let env = |name: &str| match name {
        "REGION" => Some("eu-west".to_string()),
        "API_TOKEN" => Some("tok".to_string()),
        _ => None,
    };
    let text = "{{$processEnv REGION}} {{$env.API_TOKEN}} {{$env.MISSING}}";
    let mut values = Values::default();
    resolve(
        text,
        Path::new("/nowhere"),
        &BTreeMap::new(),
        &env,
        &mut values,
    )
    .unwrap();

    assert_eq!(values.variables["$processEnv REGION"], "eu-west");
    assert_eq!(values.secrets["$env.API_TOKEN"], "tok");
    let err = variables::substitute(text, &values).unwrap_err();
    assert_eq!(err.to_string(), "unknown variables: $env.MISSING");
}

#[test]
fn reads_the_real_process_environment() {
    let mut values = Values::default();
    let env = |name: &str| std::env::var(name).ok();
    resolve(
        "{{$processEnv PATH}}",
        Path::new("/nowhere"),
        &BTreeMap::new(),
        &env,
        &mut values,
    )
    .unwrap();
    assert!(values.variables.contains_key("$processEnv PATH"));
}

#[test]
fn dotenv_values_resolve_and_are_masked() {
    let root = temp_workspace("dotenv");
    write(
        &root,
        ".env",
        "# local only\nexport API_KEY=\"abc123\"\nREGION='eu'\n\nBROKEN\n",
    );
    let text = "{{$dotenv API_KEY}}/{{$dotenv REGION}}";
    let mut values = Values::default();
    resolve(text, &root, &BTreeMap::new(), &no_env, &mut values).unwrap();

    assert_eq!(variables::substitute(text, &values).unwrap(), "abc123/eu");
    assert_eq!(
        variables::mask("key abc123", &values.secrets),
        "key {{$dotenv API_KEY}}"
    );
}

#[test]
fn credential_like_process_env_values_are_secrets() {
    let env = |name: &str| match name {
        "DB_PASSWD" => Some("hunter22".to_string()),
        "GITHUB_PAT" => Some("ghp_x".to_string()),
        "DATABASE_URL" => Some("postgres://ada:pw@db/app".to_string()),
        "REGION" => Some("eu-west-1".to_string()),
        _ => None,
    };
    let text = "{{$processEnv DB_PASSWD}} {{$env.GITHUB_PAT}} {{$processEnv DATABASE_URL}} \
                {{$processEnv REGION}}";
    let mut values = Values::default();
    resolve(
        text,
        Path::new("/nowhere"),
        &BTreeMap::new(),
        &env,
        &mut values,
    )
    .unwrap();

    assert_eq!(values.secrets.len(), 3);
    assert_eq!(values.variables["$processEnv REGION"], "eu-west-1");
}

#[test]
fn placeholders_keep_dynamic_variables_unresolved() {
    let text = "{{$prompt otp}} {{$dotenv KEY}} {{login.response.body.$.token}} \
                {{$auth.token(\"github\")}}";
    let mut values = Values::default();
    keep_as_placeholders(text, &mut values);
    assert_eq!(variables::substitute(text, &values).unwrap(), text);
}

#[test]
fn dotenv_comments_after_values_are_dropped() {
    let file = parse_dotenv(
        "PLAIN=abc # note\nQUOTED=\"a # b\" # note\nSINGLE='x y'#c\nHASH=a#b\nEMPTY= # nothing\n",
    );
    assert_eq!(file["PLAIN"], "abc");
    assert_eq!(file["QUOTED"], "a # b");
    assert_eq!(file["SINGLE"], "x y");
    assert_eq!(file["HASH"], "a#b");
    assert_eq!(file["EMPTY"], "");
}
