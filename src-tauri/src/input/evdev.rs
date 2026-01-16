//! evdev-based virtual keyboard implementation for Linux
//! 
//! This module provides native virtual keyboard functionality using the Linux evdev subsystem
//! via uinput. It creates a true virtual keyboard device that appears identical to physical
//! keyboards, providing superior compatibility compared to Wayland protocol-based tools.

#[cfg(target_os = "linux")]
use evdev::{
    uinput::VirtualDeviceBuilder, 
    AttributeSet, 
    EventType, 
    Key, 
    InputEvent,
    UinputAbsInfo,
};
use std::collections::HashMap;
use log::{info, warn, error, debug};
use crate::settings::PasteMethod;

/// Virtual keyboard device using Linux evdev/uinput
#[cfg(target_os = "linux")]
pub struct EvdevVirtualKeyboard {
    device: evdev::uinput::VirtualDevice,
    key_map: HashMap<char, Key>,
    shift_key_map: HashMap<char, Key>,
}

#[cfg(target_os = "linux")]
impl EvdevVirtualKeyboard {
    /// Create a new virtual keyboard device
    pub fn new() -> Result<Self, String> {
        debug!("Creating evdev virtual keyboard device");
        
        // Create attribute set for all keyboard keys
        let mut keys = AttributeSet::<Key>::new();
        
        // Add all standard keyboard keys
        // Letters
        for key_code in Key::KEY_A.code()..=Key::KEY_Z.code() {
            if let Ok(key) = Key::new(key_code) {
                keys.insert(key);
            }
        }
        
        // Numbers
        for key_code in Key::KEY_0.code()..=Key::KEY_9.code() {
            if let Ok(key) = Key::new(key_code) {
                keys.insert(key);
            }
        }
        
        // Essential keys for text input and shortcuts
        let essential_keys = [
            Key::KEY_SPACE, Key::KEY_ENTER, Key::KEY_TAB, Key::KEY_BACKSPACE,
            Key::KEY_LEFTCTRL, Key::KEY_RIGHTCTRL, Key::KEY_LEFTSHIFT, Key::KEY_RIGHTSHIFT,
            Key::KEY_LEFTALT, Key::KEY_RIGHTALT, Key::KEY_LEFTMETA, Key::KEY_RIGHTMETA,
            Key::KEY_INSERT, Key::KEY_DELETE, Key::KEY_HOME, Key::KEY_END,
            Key::KEY_PAGEUP, Key::KEY_PAGEDOWN, Key::KEY_UP, Key::KEY_DOWN, Key::KEY_LEFT, Key::KEY_RIGHT,
            // Punctuation and symbols
            Key::KEY_COMMA, Key::KEY_DOT, Key::KEY_SLASH, Key::KEY_SEMICOLON, Key::KEY_APOSTROPHE,
            Key::KEY_GRAVE, Key::KEY_MINUS, Key::KEY_EQUAL, Key::KEY_LEFTBRACE, Key::KEY_RIGHTBRACE,
            Key::KEY_BACKSLASH, Key::KEY_ESC, Key::KEY_F1, Key::KEY_F2, Key::KEY_F3, Key::KEY_F4,
        ];
        
        for &key in &essential_keys {
            keys.insert(key);
        }
        
        // Create the virtual device
        let device = VirtualDeviceBuilder::new()
            .map_err(|e| format!("Failed to create device builder: {}", e))?
            .name("Handy Virtual Keyboard")
            .with_keys(&keys)
            .map_err(|e| format!("Failed to add keys: {}", e))?
            .build()
            .map_err(|e| format!("Failed to build virtual device: {}", e))?;
        
        info!("Successfully created evdev virtual keyboard device");
        
        let key_map = Self::build_key_map();
        let shift_key_map = Self::build_shift_key_map();
        
        Ok(Self { 
            device, 
            key_map, 
            shift_key_map 
        })
    }
    
    /// Type a string of text using the virtual keyboard
    pub fn type_text(&mut self, text: &str) -> Result<(), String> {
        debug!("Typing text via evdev: {} chars", text.len());
        
        for ch in text.chars() {
            self.type_char(ch)?;
            // Small delay between characters to ensure proper input processing
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        
        Ok(())
    }
    
    /// Send a paste key combination (Ctrl+V, Ctrl+Shift+V, or Shift+Insert)
    pub fn send_paste_combo(&mut self, paste_method: &PasteMethod) -> Result<(), String> {
        debug!("Sending paste combo via evdev: {:?}", paste_method);
        
        match paste_method {
            PasteMethod::CtrlV => {
                self.send_key_combo(&[Key::KEY_LEFTCTRL, Key::KEY_V])?;
            }
            PasteMethod::CtrlShiftV => {
                self.send_key_combo(&[Key::KEY_LEFTCTRL, Key::KEY_LEFTSHIFT, Key::KEY_V])?;
            }
            PasteMethod::ShiftInsert => {
                self.send_key_combo(&[Key::KEY_LEFTSHIFT, Key::KEY_INSERT])?;
            }
            _ => return Err("Unsupported paste method for evdev".to_string()),
        }
        
        Ok(())
    }
    
    /// Type a single character
    fn type_char(&mut self, ch: char) -> Result<(), String> {
        if ch == '\n' {
            // Handle newline
            self.send_key_event(Key::KEY_ENTER, true)?;
            self.send_key_event(Key::KEY_ENTER, false)?;
        } else if ch == '\t' {
            // Handle tab
            self.send_key_event(Key::KEY_TAB, true)?;
            self.send_key_event(Key::KEY_TAB, false)?;
        } else if let Some(&key) = self.key_map.get(&ch) {
            // Simple character - direct key press
            self.send_key_event(key, true)?;
            self.send_key_event(key, false)?;
        } else if let Some(&key) = self.shift_key_map.get(&ch) {
            // Character that requires shift
            self.send_key_event(Key::KEY_LEFTSHIFT, true)?;
            self.send_key_event(key, true)?;
            self.send_key_event(key, false)?;
            self.send_key_event(Key::KEY_LEFTSHIFT, false)?;
        } else {
            // Unicode character - use compose sequence for best compatibility
            self.type_unicode_char(ch)?;
        }
        
        Ok(())
    }
    
    /// Send a key combination (all keys pressed simultaneously)
    fn send_key_combo(&mut self, keys: &[Key]) -> Result<(), String> {
        // Press all keys down
        for &key in keys {
            self.send_key_event(key, true)?;
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        
        // Small delay while pressed
        std::thread::sleep(std::time::Duration::from_millis(50));
        
        // Release all keys in reverse order
        for &key in keys.iter().rev() {
            self.send_key_event(key, false)?;
        }
        
        Ok(())
    }
    
    /// Send a single key event (press or release)
    fn send_key_event(&mut self, key: Key, pressed: bool) -> Result<(), String> {
        let event = InputEvent::new(
            EventType::KEY, 
            key.code(), 
            if pressed { 1 } else { 0 }
        );
        
        self.device.emit(&[event])
            .map_err(|e| format!("Failed to send key event: {}", e))?;
            
        // Send synchronization event to commit the input
        let syn_event = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
        self.device.emit(&[syn_event])
            .map_err(|e| format!("Failed to send sync event: {}", e))?;
            
        Ok(())
    }
    
    /// Handle Unicode characters using Ctrl+Shift+U sequence
    fn type_unicode_char(&mut self, ch: char) -> Result<(), String> {
        let code_point = ch as u32;
        
        // Skip control characters and very high code points
        if code_point < 32 || code_point > 0x10FFFF {
            warn!("Skipping unsupported character: U+{:04X}", code_point);
            return Ok(());
        }
        
        let hex_string = format!("{:x}", code_point);
        debug!("Typing Unicode character: {} (U+{:04X})", ch, code_point);
        
        // Use Ctrl+Shift+U sequence (standard Linux Unicode input method)
        self.send_key_event(Key::KEY_LEFTCTRL, true)?;
        self.send_key_event(Key::KEY_LEFTSHIFT, true)?;
        self.send_key_event(Key::KEY_U, true)?;
        self.send_key_event(Key::KEY_U, false)?;
        self.send_key_event(Key::KEY_LEFTSHIFT, false)?;
        self.send_key_event(Key::KEY_LEFTCTRL, false)?;
        
        // Small delay after the trigger sequence
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        // Type hex digits
        for hex_char in hex_string.chars() {
            if let Some(&key) = self.key_map.get(&hex_char) {
                self.send_key_event(key, true)?;
                self.send_key_event(key, false)?;
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        
        // Press Space or Enter to confirm (Space is more compatible)
        self.send_key_event(Key::KEY_SPACE, true)?;
        self.send_key_event(Key::KEY_SPACE, false)?;
        
        Ok(())
    }
    
    /// Build the basic character-to-key mapping
    fn build_key_map() -> HashMap<char, Key> {
        let mut map = HashMap::new();
        
        // Basic control characters
        map.insert(' ', Key::KEY_SPACE);
        
        // Lowercase letters
        for (i, ch) in ('a'..='z').enumerate() {
            if let Ok(key) = Key::new(Key::KEY_A.code() + i as u16) {
                map.insert(ch, key);
            }
        }
        
        // Numbers
        for (i, ch) in ('0'..='9').enumerate() {
            if let Ok(key) = Key::new(Key::KEY_0.code() + i as u16) {
                map.insert(ch, key);
            }
        }
        
        // Basic punctuation (unshifted)
        map.insert(',', Key::KEY_COMMA);
        map.insert('.', Key::KEY_DOT);
        map.insert(';', Key::KEY_SEMICOLON);
        map.insert('/', Key::KEY_SLASH);
        map.insert('\'', Key::KEY_APOSTROPHE);
        map.insert('`', Key::KEY_GRAVE);
        map.insert('-', Key::KEY_MINUS);
        map.insert('=', Key::KEY_EQUAL);
        map.insert('[', Key::KEY_LEFTBRACE);
        map.insert(']', Key::KEY_RIGHTBRACE);
        map.insert('\\', Key::KEY_BACKSLASH);
        
        map
    }
    
    /// Build the shift-modified character-to-key mapping
    fn build_shift_key_map() -> HashMap<char, Key> {
        let mut map = HashMap::new();
        
        // Uppercase letters
        for (i, ch) in ('A'..='Z').enumerate() {
            if let Ok(key) = Key::new(Key::KEY_A.code() + i as u16) {
                map.insert(ch, key);
            }
        }
        
        // Shifted number row
        map.insert('!', Key::KEY_1);
        map.insert('@', Key::KEY_2);
        map.insert('#', Key::KEY_3);
        map.insert('$', Key::KEY_4);
        map.insert('%', Key::KEY_5);
        map.insert('^', Key::KEY_6);
        map.insert('&', Key::KEY_7);
        map.insert('*', Key::KEY_8);
        map.insert('(', Key::KEY_9);
        map.insert(')', Key::KEY_0);
        
        // Shifted punctuation
        map.insert('<', Key::KEY_COMMA);
        map.insert('>', Key::KEY_DOT);
        map.insert(':', Key::KEY_SEMICOLON);
        map.insert('?', Key::KEY_SLASH);
        map.insert('"', Key::KEY_APOSTROPHE);
        map.insert('~', Key::KEY_GRAVE);
        map.insert('_', Key::KEY_MINUS);
        map.insert('+', Key::KEY_EQUAL);
        map.insert('{', Key::KEY_LEFTBRACE);
        map.insert('}', Key::KEY_RIGHTBRACE);
        map.insert('|', Key::KEY_BACKSLASH);
        
        map
    }
}

// Stubs for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub struct EvdevVirtualKeyboard;

#[cfg(not(target_os = "linux"))]
impl EvdevVirtualKeyboard {
    pub fn new() -> Result<Self, String> {
        Err("evdev not available on this platform".to_string())
    }
    
    pub fn type_text(&mut self, _text: &str) -> Result<(), String> {
        Err("evdev not available on this platform".to_string())
    }
    
    pub fn send_paste_combo(&mut self, _paste_method: &PasteMethod) -> Result<(), String> {
        Err("evdev not available on this platform".to_string())
    }
}

/// Check if evdev/uinput is available and accessible
pub fn is_evdev_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        // Check if uinput device is accessible
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/uinput")
        {
            Ok(_) => {
                debug!("uinput device is accessible");
                true
            }
            Err(e) => {
                debug!("uinput device not accessible: {}", e);
                false
            }
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_key_mapping() {
        let key_map = EvdevVirtualKeyboard::build_key_map();
        let shift_key_map = EvdevVirtualKeyboard::build_shift_key_map();
        
        // Test basic character mapping
        assert!(key_map.contains_key(&'a'));
        assert!(key_map.contains_key(&' '));
        assert!(key_map.contains_key(&'1'));
        assert!(key_map.contains_key(&','));
        
        // Test shift mapping
        assert!(shift_key_map.contains_key(&'A'));
        assert!(shift_key_map.contains_key(&'!'));
        assert!(shift_key_map.contains_key(&'<'));
    }
    
    #[test]
    fn test_permission_detection() {
        // Test permission detection logic
        let available = is_evdev_available();
        println!("evdev available: {}", available);

    }
    
    #[test]
    #[cfg(target_os = "linux")]
    #[ignore] // Requires uinput permissions
    fn test_evdev_device_creation() {
        match EvdevVirtualKeyboard::new() {
            Ok(_) => println!("Successfully created evdev device"),
            Err(e) => println!("Failed to create evdev device (expected without permissions): {}", e),
        }
    }
}