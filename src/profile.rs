use std::{fs::File, io::Write};

use pprof::protos::Message;

pub struct Profile<'a> {
    guard: pprof::ProfilerGuard<'a>,
}

impl<'a> Default for Profile<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Profile<'a> {
    pub fn new() -> Self {
        Self {
            guard: pprof::ProfilerGuardBuilder::default()
                .frequency(1000)
                .blocklist(&["libc", "libgcc", "pthread", "vdso"])
                .build()
                .unwrap(),
        }
    }
}

impl<'a> Drop for Profile<'a> {
    fn drop(&mut self) {
        let Ok(report) = self.guard.report().build() else {
            return;
        };

        eprintln!("Writing /tmp/profile.pb");
        {
            let mut file = File::create("/tmp/profile.pb").unwrap();
            let profile = report.pprof().unwrap();

            let mut content = Vec::new();
            profile.write_to_vec(&mut content).unwrap();
            file.write_all(&content).unwrap();
        }

        // create directory prof
        let _ = std::fs::create_dir_all("prof");
        let out = fake_tty::bash_command("pprof -png /tmp/profile.pb")
            .unwrap()
            .current_dir("prof")
            .output()
            .unwrap();
        eprint!("{}", String::from_utf8_lossy(&out.stdout));

        eprintln!("Writing /tmp/profile.svg");
        let mut file = File::create("/tmp/profile.svg").unwrap();
        report.flamegraph(&mut file).unwrap();
    }
}
