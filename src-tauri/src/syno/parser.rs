use crate::models::{AclEntry, DirNode, GroupDetail, ShareFolder, UserDetail};

pub fn parse_share_list(output: &str) -> Vec<ShareFolder> {
    let mut shares = Vec::new();
    let mut in_shares = false;

    for line in output.lines() {
        let name = line.trim();

        if name.is_empty() {
            continue;
        }

        // Detectar cuando empieza la lista real de shares
        if name.contains("Listed:") || name.ends_with("Listed") {
            in_shares = true;
            continue;
        }

        // Si todavia no llegamos a la lista, skip
        if !in_shares {
            // Algunas versiones de synoshare no tienen "Listed:", 
            // asi que tambien detectamos por la primera linea que parece un nombre
            if name.contains("Share Enum")
                || name.contains("Arguments")
                || name.contains("[0x")
                || name.contains("Copyright")
                || name.contains("Usage:")
                || name.contains("No Enumerate")
                || name.contains("Assumeing")
                || name.contains("ALL")
                || name.contains("ENC")
                || name.contains("LOCAL")
                || name.contains("USB")
                || name.contains("SATA")
                || name.contains("GLUSTER")
                || name.contains("COLD")
                || name.contains("OFFLINE")
                || name.contains("CEPH")
                || name.contains("WORM")
                || name.contains("MISSING")
                || name.contains("DEC")
                || name.contains("C2")
            {
                continue;
            }
        }

        // Skip lineas que parecen headers del comando
        if name.starts_with('[')
            || name.contains("Listed")
            || name.contains("Share Enum")
            || name.contains("Arguments")
        {
            continue;
        }

        shares.push(ShareFolder {
            name: name.to_string(),
            path: format!("/volume1/{}", name),
            description: String::new(),
        });
    }
    shares
}

pub fn parse_user_list(output: &str) -> Vec<String> {
    let mut users = Vec::new();
    let mut in_list = false;
    for line in output.lines() {
        let name = line.trim();
        if name.is_empty() {
            continue;
        }
        if name.contains("User Listed") || name.ends_with("Listed") || name.contains("Listed:") {
            in_list = true;
            continue;
        }
        if !in_list {
            if name.contains("Copyright")
                || name.contains("Usage:")
                || name.contains("No Enumerate")
                || name.contains("Assumeing")
                || name.starts_with('[')
            {
                continue;
            }
        }
        if name.contains("User Listed") || name.contains("Listed") {
            continue;
        }
        users.push(name.to_string());
    }
    users
}

pub fn parse_group_list(output: &str) -> Vec<String> {
    let mut groups = Vec::new();
    let mut in_list = false;
    for line in output.lines() {
        let name = line.trim();
        if name.is_empty() {
            continue;
        }
        if name.contains("Group Listed") || name.ends_with("Listed") || name.contains("Listed:") {
            in_list = true;
            continue;
        }
        if !in_list {
            if name.contains("Copyright")
                || name.contains("Usage:")
                || name.contains("No Enumerate")
                || name.contains("Assumeing")
                || name.starts_with('[')
            {
                continue;
            }
        }
        if name.contains("Group Listed") || name.contains("Listed") {
            continue;
        }
        groups.push(name.to_string());
    }
    groups
}

pub fn parse_dir_list(output: &str) -> Vec<DirNode> {
    let mut dirs = Vec::new();
    for line in output.lines() {
        let path = line.trim();
        if path.is_empty() {
            continue;
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        if name == "@eaDir"
            || name == "#recycle"
            || name.starts_with('@')
            || name == "."
        {
            continue;
        }
        dirs.push(DirNode {
            name: name.to_string(),
            path: path.to_string(),
            has_children: false,
        });
    }
    dirs
}

pub fn parse_acl_output(output: &str) -> Vec<AclEntry> {
    let mut entries = Vec::new();

    for line in output.lines() {
        let line = line.trim();

        // Quitar prefijo [N]
        let line = if line.starts_with('[') {
            if let Some(end) = line.find(']') {
                line[end + 1..].trim()
            } else {
                line
            }
        } else {
            line
        };

        // Quitar sufijo (level:N)
        let line = if let Some(idx) = line.rfind("(level:") {
            line[..idx].trim()
        } else {
            line
        };

        // Formato: user:name:allow/deny:permissions:flags
        // ej: user:FAriza:deny:rwxpdDaARWcCo:fd--
        // ej: group:administrators:allow:rwxpdDaARWc--:fd--
        if line.starts_with("user:") || line.starts_with("group:") {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 5 {
                let principal_type = format!("{}:{}", parts[0], parts[2]);
                let name = parts[1].to_string();
                let permissions = parts[3].to_string();
                let flags = parts[4].to_string();

                entries.push(AclEntry {
                    principal_type,
                    name,
                    permissions,
                    flags,
                });
            }
        }
    }

    entries
}

pub fn build_acl_entry_string(entry: &AclEntry) -> String {
    let (principal_type, allow_deny) = if entry.principal_type.contains(':') {
        let parts: Vec<&str> = entry.principal_type.splitn(2, ':').collect();
        (parts[0].to_string(), parts[1].to_string())
    } else {
        (entry.principal_type.clone(), "allow".to_string())
    };

    format!(
        "{}:{}:{}:{}:{}",
        principal_type, entry.name, allow_deny, entry.permissions, entry.flags
    )
}

fn extract_bracket(line: &str, prefix: &str) -> Option<String> {
    let line = line.trim();
    if !line.starts_with(prefix) {
        return None;
    }
    if let Some(start) = line.find('[') {
        if let Some(end) = line.rfind(']') {
            if start < end {
                return Some(line[start + 1..end].to_string());
            }
        }
    }
    None
}

pub fn parse_user_detail(output: &str) -> UserDetail {
    let mut name = String::new();
    let mut uid = String::new();
    let mut primary_gid = String::new();
    let mut full_name = String::new();
    let mut user_dir = String::new();
    let mut shell = String::new();
    let mut expired = false;
    let mut mail = String::new();
    let mut member_of = Vec::new();

    let mut in_members = false;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("Member Of") {
            in_members = true;
            continue;
        }

        if in_members {
            if line.starts_with('(') {
                if let Some(end) = line.find(')') {
                    let group_name = line[end + 1..].trim();
                    if !group_name.is_empty() {
                        member_of.push(group_name.to_string());
                    }
                }
            } else {
                in_members = false;
            }
        }

        if let Some(v) = extract_bracket(line, "User Name") {
            name = v;
        } else if let Some(v) = extract_bracket(line, "User uid") {
            uid = v;
        } else if let Some(v) = extract_bracket(line, "Primary gid") {
            primary_gid = v;
        } else if let Some(v) = extract_bracket(line, "Fullname") {
            full_name = v;
        } else if let Some(v) = extract_bracket(line, "User Dir") {
            user_dir = v;
        } else if let Some(v) = extract_bracket(line, "User Shell") {
            shell = v;
        } else if let Some(v) = extract_bracket(line, "Expired") {
            expired = v == "true";
        } else if let Some(v) = extract_bracket(line, "User Mail") {
            mail = v;
        }
    }

    UserDetail {
        name,
        uid,
        primary_gid,
        full_name,
        user_dir,
        shell,
        expired,
        mail,
        member_of,
    }
}

pub fn parse_group_detail(output: &str) -> GroupDetail {
    let mut name = String::new();
    let mut gid = String::new();
    let mut group_type = String::new();
    let mut members = Vec::new();

    let mut in_members = false;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("Group Members") {
            in_members = true;
            continue;
        }

        if in_members {
            if let Some(start) = line.find('[') {
                if let Some(end) = line.find(']') {
                    if start < end {
                        members.push(line[start + 1..end].to_string());
                    }
                }
            } else if !line.starts_with(|c: char| c.is_ascii_digit()) {
                in_members = false;
            }
        }

        if let Some(v) = extract_bracket(line, "Group Name") {
            name = v;
        } else if let Some(v) = extract_bracket(line, "Group ID") {
            gid = v;
        } else if let Some(v) = extract_bracket(line, "Group Type") {
            group_type = v;
        }
    }

    GroupDetail {
        name,
        gid,
        group_type,
        description: String::new(),
        members,
    }
}
