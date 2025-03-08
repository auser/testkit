use std::process::Command;

use clap::{Args as ClapArgs, Parser, Subcommand};

#[derive(Debug, Clone, ClapArgs)]
pub struct GlobalArgs {
    // #[clap(short = 'H', long, global = true, default_value = "localhost")]
    // host: Option<String>,

    // #[clap(short = 'P', long, global = true, default_value = "3306")]
    // port: Option<u16>,

    // #[clap(short, long, global = true)]
    // user: Option<String>,

    // #[clap(short, long, global = true)]
    // password: Option<String>,
    #[clap(short, long, global = true, env = "DATABASE_URL")]
    connection_url: Option<String>,

    /// The prefix for searching for databases.
    #[clap(short, long, global = true, default_value = "testkit")]
    prefix: String,

    #[clap(short, long, global = true, default_value = "postgres")]
    /// Type of the database to connect to.
    /// - `Postgres` (default)
    /// - `Mysql`
    database_type: DatabaseType,
}

#[derive(Parser, Debug)]
struct Args {
    #[clap(flatten)]
    global: GlobalArgs,

    #[clap(subcommand)]
    operation: Operation,
}

#[derive(Debug, Subcommand)]
enum Operation {
    /// List all the testkit databases
    List,
    /// Reset the testkit databases
    Reset,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum DatabaseType {
    #[clap(alias = "pg")]
    Postgres,
    #[clap(alias = "mysql")]
    Mysql,
}

fn main() {
    let args = Args::parse();
    match args.operation {
        Operation::List => list_databases(&args.global),
        Operation::Reset => reset_databases(&args.global),
    }
}

fn list_databases(args: &GlobalArgs) {
    let database_type = &args.database_type;
    let connection_url = args.connection_url.clone().unwrap_or_default();
    let connection = get_root_url(&connection_url);

    match database_type {
        DatabaseType::Postgres => list_postgres_databases(&connection, &args.prefix),
        DatabaseType::Mysql => list_mysql_databases(&connection, &args.prefix),
    }
}

fn reset_databases(args: &GlobalArgs) {
    // Parse the connection URL if provided
    let connection = match &args.connection_url {
        Some(url) => get_root_url(url),
        None => {
            println!("No connection URL provided. Using defaults.");
            DBConnection {
                protocol: None,
                host: "localhost".to_string(),
                port: None,
                user: None,
                password: None,
                database: None,
            }
        }
    };

    match args.database_type {
        DatabaseType::Postgres => reset_postgres_databases(&connection, &args.prefix),
        DatabaseType::Mysql => reset_mysql_databases(&connection, &args.prefix),
    }
}

fn reset_postgres_databases(connection: &DBConnection, prefix: &str) {
    // First, get a list of all databases matching the prefix
    let query = format!(
        "SELECT datname FROM pg_database WHERE datname LIKE '{}%';",
        prefix
    );

    match psql_command(connection, &query) {
        Ok(output) => {
            let databases = output
                .lines()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<&str>>();

            if databases.is_empty() {
                println!("No databases found with prefix: {}", prefix);
                return;
            }

            for db in &databases {
                println!("  - {}", db);
            }

            let mut failed = false;

            // Drop each database individually
            for db in databases {
                let drop_query = format!("DROP DATABASE \"{}\";", db);

                match psql_command(connection, &drop_query) {
                    Ok(_) => println!("Successfully dropped database: {}", db),
                    Err(e) => {
                        println!("Failed to drop database {}: {}", db, e);
                        failed = true;
                    }
                }
            }

            if failed {
                println!("Some databases could not be dropped. Check the output above.");
            } else {
                println!("Successfully dropped all databases with prefix: {}", prefix);
            }
        }
        Err(e) => println!("Error listing databases: {}", e),
    }
}

fn reset_mysql_databases(connection: &DBConnection, prefix: &str) {
    // First, get a list of all databases matching the prefix using information_schema
    let query = format!(
        "SELECT schema_name FROM information_schema.schemata WHERE schema_name LIKE '{}%'",
        prefix
    );
    println!("Finding MySQL databases with prefix: {}", prefix);

    match mysql_command(connection, &query) {
        Ok(output) => {
            // Process the schema names
            let databases = output
                .lines()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<&str>>();

            if databases.is_empty() {
                println!("No databases found with prefix: {}", prefix);
                return;
            }

            println!("Found {} databases to drop:", databases.len());
            for db in &databases {
                println!("  - {}", db);
            }

            println!("Starting to drop databases...");
            let mut failed = false;

            // Drop each database individually
            for db in databases {
                let drop_query = format!("DROP DATABASE `{}`", db);
                println!("Executing: {}", drop_query);

                match mysql_command(connection, &drop_query) {
                    Ok(_) => println!("Successfully dropped database: {}", db),
                    Err(e) => {
                        println!("Failed to drop database {}: {}", db, e);
                        failed = true;
                    }
                }
            }

            if failed {
                println!("Some databases could not be dropped. Check the output above.");
            } else {
                println!("Successfully dropped all databases with prefix: {}", prefix);
            }
        }
        Err(e) => println!("Error listing databases: {}", e),
    }
}

fn list_postgres_databases(connection: &DBConnection, prefix: &str) {
    // Format the SQL query with the prefix directly in the string
    let query = format!(
        "SELECT datname FROM pg_database WHERE datname LIKE '{}%'",
        prefix
    );

    let output = psql_command(connection, &query);

    match output {
        Ok(output) => {
            let databases = output
                .lines()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<&str>>();

            if databases.is_empty() {
                println!("No databases found with prefix: {}", prefix);
            } else {
                println!(
                    "Found {} databases with prefix '{}':",
                    databases.len(),
                    prefix
                );
                for db in databases {
                    println!("  - {}", db);
                }
            }
        }
        Err(e) => println!("Error parsing psql output: {}", e),
    }
}

fn list_mysql_databases(connection: &DBConnection, prefix: &str) {
    // Format the query to use information_schema instead of SHOW DATABASES
    let query = format!(
        "SELECT schema_name FROM information_schema.schemata WHERE schema_name LIKE '{}%'",
        prefix
    );

    match mysql_command(connection, &query) {
        Ok(output) => {
            // Process the output, which is just schema names without headers
            let databases = output
                .lines()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<&str>>();

            if databases.is_empty() {
                println!("No databases found with prefix: {}", prefix);
            } else {
                println!(
                    "Found {} databases with prefix '{}':",
                    databases.len(),
                    prefix
                );
                for db in databases {
                    println!("  - {}", db);
                }
            }
        }
        Err(e) => println!("Error executing MySQL command: {}", e),
    }
}

fn psql_command(connection: &DBConnection, query: &str) -> Result<String, String> {
    // First, print all connection details (with password masked)
    println!("Connection details:");
    println!("  - Host: {}", connection.host);
    println!("  - Port: {:?}", connection.port);
    println!("  - User: {:?}", connection.user);
    println!("  - Has password: {}", connection.password.is_some());
    println!("  - Database: {:?}", connection.database);

    let args = vec![
        "-h".to_string(),
        connection.host.clone(),
        "-p".to_string(),
        connection.port.unwrap_or(5432).to_string(),
        "-U".to_string(),
        connection
            .user
            .as_ref()
            .unwrap_or(&"postgres".to_string())
            .clone(),
        "-t".to_string(), // Tuples only, no headers
        "-c".to_string(),
        query.to_string(),
    ];

    // Print the actual command for debugging
    println!("Debug: Running command: psql {}", args.join(" "));

    // Create command and set environment variable for password if needed
    let mut cmd = Command::new("psql");
    if let Some(password) = connection.password.as_ref() {
        cmd.env("PGPASSWORD", password);
        println!("  (Using PGPASSWORD environment variable)");
    }

    // Try verbose command execution with error output capture
    let output = cmd.args(&args).output();

    match output {
        Ok(cmd_output) => {
            let stdout = String::from_utf8_lossy(&cmd_output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&cmd_output.stderr).to_string();

            if !stderr.is_empty() {
                println!("PostgreSQL stderr: {}", stderr);
            }

            if cmd_output.status.success() {
                Ok(stdout)
            } else {
                Err(format!("PostgreSQL error: {}", stderr))
            }
        }
        Err(e) => Err(format!("Failed to execute psql command: {}", e)),
    }
}

fn mysql_command(connection: &DBConnection, query: &str) -> Result<String, String> {
    let mut args = vec![
        "-h".to_string(),
        connection.host.clone(),
        "-u".to_string(),
        connection
            .user
            .as_ref()
            .unwrap_or(&"root".to_string())
            .clone(),
        "-P".to_string(),
        connection.port.unwrap_or(3306).to_string(),
    ];

    // Handle password without separating -p and the password
    // MySQL requires -p immediately followed by password with no spaces
    if let Some(password) = connection.password.as_ref() {
        // Check if the password contains any special characters
        if password.contains(' ') || password.contains('\'') || password.contains('"') {
            // For passwords with special chars, use single quotes but escape any internal single quotes
            args.push(format!("-p{}", password.replace('\'', "'\\''")));
        } else {
            args.push(format!("-p{}", password));
        }
    }

    // Make sure we're connecting to mysql without a database specified initially
    // args.push("--no-defaults".to_string());

    // Add the query
    args.push("-e".to_string());
    args.push(query.to_string());

    // Print the actual command for debugging (without showing password)
    let debug_args = args
        .iter()
        .map(|arg| {
            if arg.starts_with("-p") && arg.len() > 2 {
                "-p***".to_string()
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>();
    println!("Debug: Running command: mysql {}", debug_args.join(" "));

    // Try verbose command execution with error output capture
    let output = Command::new("mysql").args(&args).output();

    match output {
        Ok(cmd_output) => {
            let stdout = String::from_utf8_lossy(&cmd_output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&cmd_output.stderr).to_string();

            if !stderr.is_empty() {
                println!("MySQL stderr: {}", stderr);
            }

            if cmd_output.status.success() {
                Ok(stdout)
            } else {
                Err(format!("MySQL error: {}", stderr))
            }
        }
        Err(e) => Err(format!("Failed to execute mysql command: {}", e)),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct DBConnection {
    protocol: Option<String>,
    host: String,
    port: Option<u16>,
    user: Option<String>,
    password: Option<String>,
    database: Option<String>,
}

fn get_root_url(url: &str) -> DBConnection {
    let (protocol, rest) = match url.split_once("://") {
        Some((protocol, rest)) => (Some(protocol.to_string()), rest),
        None => (None, url),
    };

    // Handle authentication (user:password@)
    let (auth_part, host_part) = match rest.split_once('@') {
        Some((auth, host)) => (Some(auth), host),
        None => (None, rest),
    };

    // Extract user and password if auth part exists
    let (user, password) = match auth_part {
        Some(auth) => match auth.split_once(':') {
            Some((u, p)) => (Some(u.to_string()), Some(p.to_string())),
            None => (Some(auth.to_string()), None),
        },
        None => (None, None),
    };

    // Handle host, port and database part
    let host_port_db: Vec<&str> = host_part.split('/').collect();
    let host_port = host_port_db[0];

    // Get the database name (if any)
    let database = if host_port_db.len() > 1 {
        let db = host_port_db[1];
        if db.is_empty() {
            None
        } else {
            Some(db.to_string())
        }
    } else {
        None
    };

    // Extract host and port
    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().ok()),
        None => (host_port.to_string(), None),
    };

    DBConnection {
        protocol,
        host,
        port,
        user,
        password,
        database,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_root_url() {
        assert_eq!(
            get_root_url("mysql://root:root@mysql:3306/mysql"),
            DBConnection {
                protocol: Some("mysql".to_string()),
                host: "mysql".to_string(),
                port: Some(3306),
                user: Some("root".to_string()),
                password: Some("root".to_string()),
                database: Some("mysql".to_string()),
            }
        );
        assert_eq!(
            get_root_url("postgres://user:pass@localhost:5432/db"),
            DBConnection {
                protocol: Some("postgres".to_string()),
                host: "localhost".to_string(),
                port: Some(5432),
                user: Some("user".to_string()),
                password: Some("pass".to_string()),
                database: Some("db".to_string()),
            }
        );
        assert_eq!(
            get_root_url("mysql://root:root@mysql:3306"),
            DBConnection {
                protocol: Some("mysql".to_string()),
                host: "mysql".to_string(),
                port: Some(3306),
                user: Some("root".to_string()),
                password: Some("root".to_string()),
                database: None,
            }
        );
    }
}
