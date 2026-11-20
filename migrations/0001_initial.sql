PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS laboratories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    location TEXT,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS asset_categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT
);

CREATE TABLE IF NOT EXISTS assets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_tag TEXT NOT NULL UNIQUE,
    laboratory_id INTEGER NOT NULL REFERENCES laboratories(id),
    category_id INTEGER REFERENCES asset_categories(id),
    brand TEXT,
    model TEXT,
    serial_number TEXT,
    hostname TEXT,
    ip_address TEXT,
    status TEXT NOT NULL DEFAULT 'working'
        CHECK (status IN ('working','needs_repair','under_maintenance','retired','missing')),
    purchase_date TEXT,
    warranty_end TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    full_name TEXT NOT NULL,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('administrator','custodian','technician','viewer')),
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS maintenance_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_id INTEGER NOT NULL REFERENCES assets(id),
    reported_by INTEGER REFERENCES users(id),
    issue TEXT NOT NULL,
    action_taken TEXT,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open','in_progress','resolved','cancelled')),
    reported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    resolved_at TEXT
);

CREATE TABLE IF NOT EXISTS monitoring_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_tag TEXT NOT NULL,
    hostname TEXT,
    os_version TEXT,
    cpu_percent REAL,
    memory_percent REAL,
    disk_free_bytes INTEGER,
    uptime_seconds INTEGER,
    received_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_assets_lab ON assets(laboratory_id);
CREATE INDEX IF NOT EXISTS idx_assets_status ON assets(status);
CREATE INDEX IF NOT EXISTS idx_maintenance_asset ON maintenance_logs(asset_id);
CREATE INDEX IF NOT EXISTS idx_monitoring_asset_time ON monitoring_logs(asset_tag, received_at);

INSERT OR IGNORE INTO laboratories (code, name) VALUES
('LAB-01','Computer Laboratory 1'),
('LAB-02','Computer Laboratory 2'),
('LAB-03','Computer Laboratory 3'),
('LAB-04','Computer Laboratory 4'),
('LAB-05','Computer Laboratory 5'),
('LAB-06','Computer Laboratory 6');

INSERT OR IGNORE INTO asset_categories (name, description) VALUES
('Desktop Computer','Laboratory desktop workstation'),
('Monitor','Computer display'),
('Keyboard','Computer keyboard'),
('Mouse','Computer mouse'),
('Network Equipment','Switch, router or access point'),
('Printer','Printer or multifunction device');
