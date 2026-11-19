CREATE DATABASE IF NOT EXISTS clamms_db CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
USE clamms_db;

CREATE TABLE IF NOT EXISTS users (
 id INT AUTO_INCREMENT PRIMARY KEY,
 name VARCHAR(120) NOT NULL,
 username VARCHAR(80) NOT NULL UNIQUE,
 password_hash VARCHAR(255) NOT NULL,
 role ENUM('admin','custodian') NOT NULL DEFAULT 'custodian',
 created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS laboratories (
 id INT AUTO_INCREMENT PRIMARY KEY,
 name VARCHAR(120) NOT NULL UNIQUE,
 room_code VARCHAR(40) NULL,
 computer_count INT NOT NULL DEFAULT 0,
 description TEXT NULL
);

CREATE TABLE IF NOT EXISTS asset_categories (
 id INT AUTO_INCREMENT PRIMARY KEY,
 name VARCHAR(100) NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS assets (
 id INT AUTO_INCREMENT PRIMARY KEY,
 lab_id INT NOT NULL,
 category_id INT NULL,
 asset_tag VARCHAR(80) NOT NULL UNIQUE,
 name VARCHAR(160) NOT NULL,
 serial_number VARCHAR(120) NULL,
 status ENUM('working','needs_repair','under_maintenance','retired') NOT NULL DEFAULT 'working',
 notes TEXT NULL,
 last_seen_at DATETIME NULL,
 created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
 FOREIGN KEY (lab_id) REFERENCES laboratories(id),
 FOREIGN KEY (category_id) REFERENCES asset_categories(id)
);

CREATE TABLE IF NOT EXISTS maintenance_logs (
 id INT AUTO_INCREMENT PRIMARY KEY,
 asset_id INT NOT NULL,
 reported_by INT NULL,
 issue TEXT NOT NULL,
 action_taken TEXT NULL,
 status ENUM('open','in_progress','resolved') NOT NULL DEFAULT 'open',
 reported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
 resolved_at DATETIME NULL,
 FOREIGN KEY (asset_id) REFERENCES assets(id),
 FOREIGN KEY (reported_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS monitoring_logs (
 id BIGINT AUTO_INCREMENT PRIMARY KEY,
 asset_id INT NOT NULL,
 status VARCHAR(40) NOT NULL,
 ip_address VARCHAR(45) NULL,
 details JSON NULL,
 recorded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
 INDEX(asset_id, recorded_at),
 FOREIGN KEY (asset_id) REFERENCES assets(id)
);

CREATE TABLE IF NOT EXISTS audit_logs (
 id BIGINT AUTO_INCREMENT PRIMARY KEY,
 user_id INT NULL,
 action VARCHAR(120) NOT NULL,
 details TEXT NULL,
 created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
 FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
);

INSERT IGNORE INTO laboratories(name, room_code, computer_count) VALUES
('Computer Laboratory 1','CL-1',0),('Computer Laboratory 2','CL-2',0),
('Computer Laboratory 3','CL-3',0),('Computer Laboratory 4','CL-4',0),
('Computer Laboratory 5','CL-5',0),('Computer Laboratory 6','CL-6',0);

INSERT IGNORE INTO asset_categories(name) VALUES
('Desktop Computer'),('Monitor'),('Keyboard'),('Mouse'),('Printer'),('Network Equipment'),('Other');
