use std::time::Duration;
use tauri::{AppHandle, Emitter};
use serde::Serialize;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};


#[derive(Clone, Serialize)]
struct BatteryPayload {
    state: String,
    percent: u8,
}

pub fn start_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        
        unsafe { let _ = CoInitializeEx(None, COINIT_MULTITHREADED); }
        
        let mut last_volume = get_system_volume().unwrap_or(-1.0);
        let mut last_power_state = 255;    
        let mut last_battery_percent = 255; 

        
        if let Some((ac_status, battery_percent)) = get_power_status() {
            last_power_state = ac_status;
            last_battery_percent = battery_percent;
        }

        loop {
            std::thread::sleep(Duration::from_millis(800)); 

            
            if let Some(current_volume) = get_system_volume() {
                if (current_volume - last_volume).abs() > 0.01 && last_volume != -1.0 {
                    let vol_percent = (current_volume * 100.0).round() as i32;
                    let _ = app.emit("system-event", format!("当前系统音量 {}%", vol_percent));
                }
                last_volume = current_volume;
            }

            
            if let Some((current_power, current_percent)) = get_power_status() {
                
                
                if current_power != last_power_state && last_power_state != 255 {
                    if current_power == 1 {
                        
                        let _ = app.emit("battery-event", BatteryPayload {
                            state: "charging".to_string(),
                            percent: current_percent,
                        });
                    } else if current_power == 0 {
                        
                        let _ = app.emit("system-event", "正在使用电池供电");
                    }
                }

                
                if current_power == 0 && current_percent < last_battery_percent {
                    
                    if current_percent <= 20 && [20, 15, 10, 5].contains(&current_percent) {
                        let _ = app.emit("battery-event", BatteryPayload {
                            state: "discharging".to_string(),
                            percent: current_percent,
                        });
                    }
                }

                last_power_state = current_power;
                last_battery_percent = current_percent;
            }
        }
    });
}


fn get_system_volume() -> Option<f32> {
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let endpoint_volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None).ok()?;
        
        let volume = endpoint_volume.GetMasterVolumeLevelScalar().ok()?;
        Some(volume)
    }
}


fn get_power_status() -> Option<(u8, u8)> {
    unsafe {
        let mut status: SYSTEM_POWER_STATUS = std::mem::zeroed();
        if GetSystemPowerStatus(&mut status).is_ok() {
            
            
            
            Some((status.ACLineStatus, status.BatteryLifePercent))
        } else {
            None
        }
    }
}