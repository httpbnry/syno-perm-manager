export interface Connection {
  id: number | null
  name: string
  host: string
  port: number
  username: string
  use_sudo: boolean
  has_password: boolean
  auth_method: string
  key_path: string | null
}

export interface ConnectionInput {
  name: string
  host: string
  port: number
  username: string
  password: string
  use_sudo: boolean
  auth_method: string
  key_path: string | null
  key_passphrase: string | null
}

export interface ShareFolder {
  name: string
  path: string
  description: string
}

export interface DirNode {
  name: string
  path: string
  has_children: boolean
}

export interface AclEntry {
  principal_type: string
  name: string
  permissions: string
  flags: string
}

export interface AclDiff {
  path: string
  principal_type: string
  name: string
  old_permissions: string | null
  new_permissions: string | null
  action: string
}

export interface AuditLog {
  id: number
  timestamp: string
  connection_name: string
  action: string
  path: string
  details: string
  success: boolean
  snapshot_ids: string
}

export interface ApplyResult {
  success: boolean
  paths_modified: number
  errors: string[]
}

export interface ApplyRequest {
  paths: string[]
  entries: AclEntry[]
  recursive: boolean
}

export interface SnapshotInfo {
  id: number
  timestamp: string
  connection_name: string
  path: string
  recursive: boolean
}

export interface PathAcl {
  path: string
  entries: AclEntry[]
}

export type PermColor = 'red' | 'orange' | 'green'

export interface SelectedPrincipal {
  type: 'user' | 'group'
  name: string
}

export interface UserDetail {
  name: string
  uid: string
  primary_gid: string
  full_name: string
  user_dir: string
  shell: string
  expired: boolean
  mail: string
  member_of: string[]
}

export interface GroupDetail {
  name: string
  gid: string
  group_type: string
  description: string
  members: string[]
}

export interface CreateUserInput {
  username: string
  password: string
  full_name: string
  expired: boolean
  mail: string
  privilege: number
}

export interface CreateGroupInput {
  name: string
  members: string[]
}

export interface DashboardStats {
  user_count: number
  group_count: number
  share_count: number
  recent_logs: AuditLog[]
  total_changes: number
  successful_changes: number
  failed_changes: number
}

export interface PermMatrixCell {
  path: string
  color: string
  permissions: string
}

export interface PermMatrixRow {
  principal: string
  principal_type: string
  cells: PermMatrixCell[]
}

export interface PermMatrix {
  paths: string[]
  rows: PermMatrixRow[]
}

export interface StartupResult {
  ran: boolean
  output: string
  success: boolean
}

export interface AppConfig {
  volume_path: string
  synoacltool_path: string
  synoshare_path: string
  synouser_path: string
  synogroup_path: string
  find_path: string
  excluded_folders: string
  ssh_timeout_secs: number
  keepalive_secs: number
  log_retention_days: number
  snapshot_retention_count: number
  password_length: number
  password_special_chars: boolean
  username_format: string
  description_template: string
  theme: string
  language: string
}
