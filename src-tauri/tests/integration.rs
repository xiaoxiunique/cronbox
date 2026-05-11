#[cfg(test)]
mod integration_tests {
    use cronbox_lib::executor;
    use cronbox_lib::models::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn create_temp_script(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cronbox-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[tokio::test]
    async fn test_bash_execution() {
        let path = create_temp_script("hello.sh", "#!/bin/bash\necho 'hello from bash'");
        let result = executor::execute(&path, ScriptLanguage::Bash, "{}").await;
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello from bash"));
        println!("Bash: {}ms — {}", result.duration_ms, result.stdout.trim());
    }

    #[tokio::test]
    async fn test_bash_with_args() {
        let path = create_temp_script("args.sh", "#!/bin/bash\necho \"greeting=$greeting\"");
        let result = executor::execute(&path, ScriptLanguage::Bash, r#"{"greeting": "hi"}"#).await;
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(result.stdout.contains("greeting=hi"));
    }

    #[tokio::test]
    async fn test_bash_failure() {
        let path = create_temp_script("fail.sh", "#!/bin/bash\necho 'about to fail'\nexit 1");
        let result = executor::execute(&path, ScriptLanguage::Bash, "{}").await;
        assert_ne!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_python_execution() {
        let path = create_temp_script(
            "hello.py",
            "import json, os\nprint('hello from python')\nprint(json.dumps({'answer': 42}))",
        );
        let result = executor::execute(&path, ScriptLanguage::Python, "{}").await;
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(result.stdout.contains("hello from python"));
        assert!(result.result.is_some());
        println!(
            "Python: {}ms — result={:?}",
            result.duration_ms, result.result
        );
    }

    #[tokio::test]
    async fn test_bun_execution() {
        if std::process::Command::new("which")
            .arg("bun")
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
        {
            println!("Skipping: bun not installed");
            return;
        }
        let path = create_temp_script(
            "hello.ts",
            "console.log('hello from bun')\nconsole.log(JSON.stringify({ts: true}))",
        );
        let result = executor::execute(&path, ScriptLanguage::Bun, "{}").await;
        assert_eq!(result.exit_code, 0, "stderr: {}", result.stderr);
        assert!(result.stdout.contains("hello from bun"));
        println!("Bun: {}ms — result={:?}", result.duration_ms, result.result);
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(
            ScriptLanguage::from_extension("backup.sh"),
            Some(ScriptLanguage::Bash)
        );
        assert_eq!(
            ScriptLanguage::from_extension("deploy.py"),
            Some(ScriptLanguage::Python)
        );
        assert_eq!(
            ScriptLanguage::from_extension("api.ts"),
            Some(ScriptLanguage::Bun)
        );
        assert_eq!(
            ScriptLanguage::from_extension("query.sql"),
            Some(ScriptLanguage::PostgreSql)
        );
        assert_eq!(ScriptLanguage::from_extension("readme.md"), None);
        assert_eq!(
            ScriptLanguage::from_extension("script.js"),
            Some(ScriptLanguage::Bun)
        );
    }

    #[test]
    fn test_directory_scan() {
        use cronbox_lib::state::AppState;

        let dir = std::env::temp_dir().join(format!("cronbox-scan-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("subdir")).unwrap();
        std::fs::write(dir.join("backup.sh"), "#!/bin/bash\necho ok").unwrap();
        std::fs::write(
            dir.join("deploy.py"),
            "print('ok')\nif __name__ == \"__main__\":\n    pass\n",
        )
        .unwrap();
        std::fs::write(dir.join("helper.py"), "def helper():\n    return 1\n").unwrap();
        std::fs::write(
            dir.join("subdir/check.ts"),
            "if (import.meta.main) console.log('ok')",
        )
        .unwrap();
        std::fs::write(dir.join("subdir/util.ts"), "export const value = 1").unwrap();
        std::fs::write(dir.join("readme.md"), "# not a script").unwrap();
        std::fs::write(dir.join(".hidden.sh"), "echo hidden").unwrap();

        let db_path = dir.join("test.db");
        let state = AppState::new(db_path).unwrap();

        // Add work dir via DB
        {
            state
                .db
                .lock()
                .unwrap()
                .add_work_dir(dir.to_str().unwrap())
                .unwrap();
        }

        let scripts = state.scan_scripts();
        assert_eq!(scripts.len(), 3);
        let names: Vec<&str> = scripts.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"backup.sh"));
        assert!(names.contains(&"deploy.py"));
        assert!(names.contains(&"check.ts"));
        assert!(!names.contains(&"helper.py"));
        assert!(!names.contains(&"util.ts"));
        // Verify base_dir is set
        assert!(scripts.iter().all(|s| s.base_dir == dir.to_str().unwrap()));
        assert!(scripts.iter().all(|s| !s.alias.is_empty()));
        assert!(scripts
            .iter()
            .any(|s| s.name == "backup.sh" && s.alias == "backup"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_full_engine_lifecycle() {
        use cronbox_lib::state::AppState;

        let dir = std::env::temp_dir().join(format!("cronbox-e2e-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hello.sh"), "#!/bin/bash\necho 'engine test ok'").unwrap();

        let db_path = dir.join("test.db");
        let state = AppState::new(db_path).unwrap();
        {
            state
                .db
                .lock()
                .unwrap()
                .add_work_dir(dir.to_str().unwrap())
                .unwrap();
        }

        let scripts = state.scan_scripts();
        assert_eq!(scripts.len(), 1);

        // Create schedule via db
        {
            let db = state.db.lock().unwrap();
            let s = db
                .create_schedule("hello.sh", dir.to_str().unwrap(), "0 * * * *", "UTC", "{}")
                .unwrap();
            let next = cronbox_lib::scheduler::calculate_next_run("0 * * * *", "UTC").unwrap();
            db.update_schedule_next_run(&s.id, &next).unwrap();
            assert!(db.get_schedule(&s.id).unwrap().next_run_at.is_some());
            assert_eq!(
                db.get_schedule(&s.id).unwrap().base_dir,
                dir.to_str().unwrap()
            );
        }

        // Run manually via executor
        let rt = tokio::runtime::Runtime::new().unwrap();
        let full_path = dir.join("hello.sh");
        let result = rt.block_on(cronbox_lib::executor::execute(
            &full_path,
            ScriptLanguage::Bash,
            "{}",
        ));
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("engine test ok"));
        println!(
            "E2E passed: output='{}' duration={}ms",
            result.stdout.trim(),
            result.duration_ms
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
