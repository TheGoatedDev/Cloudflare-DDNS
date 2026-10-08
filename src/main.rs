use std::env;

use dotenvy::dotenv;


struct Config {
    token: String,
    zone_id: String,
    record_name: String,
}

fn require_env(key: &str) -> Result<String, Box<dyn std::error::Error>> {
    env::var(key).map_err(|_| format!("Environment variable {key} is required").into())
}

fn config() -> Result<Config, Box<dyn std::error::Error>> {
    Ok(Config {
        token: require_env("CF_API_TOKEN")?,
        zone_id: require_env("CF_ZONE_ID")?,
        record_name: require_env("CF_RECORD_NAME")?,
    })
}

fn public_ip() -> Result<String, Box<dyn std::error::Error>> {
    let response = ureq::get("https://api.ipify.org").call()?.body_mut().read_to_string()?;
    Ok(response)
}

#[derive(serde::Deserialize)]
struct CFDnsRecord {
    id: String,
    #[serde(rename = "type")]
    rtype: String,
    content: String,
}

#[derive(serde::Deserialize)]
struct CFListDnsRecordsResponse {
    success: bool,
    result: Vec<CFDnsRecord>,
}

fn list_dns_records(config: &Config) -> Result<Vec<CFDnsRecord>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.cloudflare.com/client/v4/zones/{}/dns_records?type=A&name={}",
        config.zone_id, config.record_name
    );
    let body = ureq::get(&url)
        .header("Authorization", &format!("Bearer {}", config.token))
        .call()?
        .body_mut()
        .read_to_string()?;
    
    let response: CFListDnsRecordsResponse = serde_json::from_str(&body)?;
    
    if !response.success {
        return Err("Failed to list DNS records".into());
    }

    Ok(response.result)
}

fn update_dns_record(config: &Config, record_id: &str, new_ip: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
        config.zone_id, record_id
    );
    let body = serde_json::json!({
        "content": new_ip,
    });
    
    let response = ureq::put(&url)
        .header("Authorization", &format!("Bearer {}", config.token))
        .header("Content-Type", "application/json")
        .send(body.to_string())?
        .body_mut()
        .read_to_string()?;
    
    let response: CFListDnsRecordsResponse = serde_json::from_str(&response)?;
    
    if !response.success {
        return Err("Failed to update DNS record".into());
    }

    Ok(())
}

fn create_dns_record(config: &Config, new_ip: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
        config.zone_id
    );
    let body = serde_json::json!({
        "type": "A",
        "name": config.record_name,
        "content": new_ip,
        "ttl": 1,
        "proxied": true
    });
    
    let response = ureq::post(&url)
        .header("Authorization", &format!("Bearer {}", config.token))
        .header("Content-Type", "application/json")
        .send(body.to_string())?
        .body_mut()
        .read_to_string()?;
    
    let response: CFListDnsRecordsResponse = serde_json::from_str(&response)?;
    
    if !response.success {
        return Err("Failed to create DNS record".into());
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    let config = config()?;
    let public_ip = public_ip()?;
    println!("Public IP: {}", public_ip);
    println!("Zone ID: {}", config.zone_id);
    println!("Record Name: {}", config.record_name);

    println!("Listing DNS records for {}...", config.record_name);
    let records = list_dns_records(&config)?;
    
    let record = records.into_iter().next();

    match record {
        Some(record) => {
            println!("Found existing A record: {} with IP {}", record.id, record.content);
            if record.content != public_ip {
                println!("Updating A record {} to new IP {}...", record.id, public_ip);
                update_dns_record(&config, &record.id, &public_ip)?;
                println!("A record updated successfully.");
            } else {
                println!("A record is already up to date with the current public IP.");
            }
        },
        None => {
            println!("No existing A record found for {}. Creating new record with IP {}...", config.record_name, public_ip);
            create_dns_record(&config, &public_ip)?;
            println!("A record created successfully.");
        }
        
    }

    Ok(())
}
