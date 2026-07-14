pub fn shell_escape(input: &str) -> String {
    format!("'{}'", input.replace("'", "'\"'\"'"))
}

pub fn extract_error(res: &crate::ssh::client::CommandResult) -> String {
    let msg = if res.stderr.is_empty() {
        res.stdout.clone()
    } else {
        res.stderr.clone()
    };
    msg.lines()
        .filter(|l| {
            !l.contains("Could not chdir")
                && !l.contains("[sudo]")
                && !l.is_empty()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

pub const SYNOSHARE: &str = "/usr/syno/sbin/synoshare";
pub const SYNOUSER: &str = "/usr/syno/sbin/synouser";
pub const SYNOGROUP: &str = "/usr/syno/sbin/synogroup";
pub const SYNOACLTOOL: &str = "/usr/syno/bin/synoacltool";
pub const FIND: &str = "/bin/find";
