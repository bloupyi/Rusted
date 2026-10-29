#!/usr/bin/env python3
"""Imported-pack protocol and forced parent shutdown, using only original process fixtures."""
import hashlib
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time
import unittest
from test_horizon_bridge import BINARY_DIR, BRIDGE, PEER, ROOT, TOKEN, WINE, native_path, REVISION

PARENT=BINARY_DIR/'examples'/('bootstrap_parent.exe' if WINE or os.name=='nt' else 'bootstrap_parent')


class BootstrapProcessTest(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(prefix="Rusted import O'Neil #1 ")
        self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name);self.game=self.root/'minecraft';self.runtime=self.game/'rusted'
        self.runtime.mkdir(parents=True);(self.game/'mods').mkdir()
        self.bridge=self.runtime/'rusted-launch.exe';shutil.copy2(BRIDGE,self.bridge)
        body=(ROOT/'build/bootstrap/rusted-bootstrap.jar').read_bytes()
        (self.game/'mods/rusted-bootstrap.jar').write_bytes(body)
        (self.runtime/'bootstrap.json').write_text(json.dumps({'schema':1,'jar_sha256':hashlib.sha256(body).hexdigest()}))
        (self.runtime/'resources').mkdir();(self.runtime/'resources/horizon-resources.json').write_text('{"minecraft":"26.3"}')
        self.config={'schema':1,'mode':'auto','horizon_revision':REVISION,'minecraft_version':'26.3','executable':native_path(PEER),'resources':'resources','data_directory':'native-data'}
        self.input={'schema':1,'parent_pid':1,'java':'java','minecraft_version':'26.3','game_directory':native_path(self.game),'classpath':'fixture classpath',
            'game_args':['--username','BridgeFixture','--uuid','123456781234123412341234567890ab','--accessToken',TOKEN,'--version','fabric-loader-0.20.1-26.3','--gameDir',native_path(self.game)]}
        self.path=self.root/'private-input.json'
        self.env=os.environ.copy();self.env['TMP']=self.env['TEMP']=native_path(self.root)
        if not WINE:self.env['TMPDIR']=str(self.root)

    def command(self):
        self.path.write_text(json.dumps(self.input));(self.runtime/'rusted-runtime.json').write_text(json.dumps(self.config))
        return ([WINE] if WINE else [])+[native_path(PARENT),native_path(self.bridge),native_path(self.path)]

    def invoke(self):
        result=subprocess.run(self.command(),capture_output=True,text=True,timeout=25,env=self.env)
        self.assertNotIn(TOKEN,result.stdout+result.stderr)
        self.assertEqual(list(self.root.glob('rusted-session-*')),[])
        return result

    def test_imported_native_pack_keeps_session_exit_and_windows_paths(self):
        result=self.invoke();self.assertEqual(result.returncode,37,result.stderr)
        self.assertIn('RUSTED_BOOTSTRAP_NATIVE',result.stdout);self.assertIn('TEST_PEER_NATIVE_OK',result.stdout)

    def test_java_mods_continue_jvm_without_creating_native_data(self):
        (self.game/'mods/other.jar').write_bytes(b'OWN_MOD_FIXTURE_ONLY')
        result=self.invoke();self.assertEqual(result.returncode,0,result.stderr)
        self.assertIn('RUSTED_BOOTSTRAP_JAVA',result.stdout);self.assertNotIn('TEST_PEER_NATIVE_OK',result.stdout)
        self.assertFalse((self.runtime/'native-data').exists())

    def test_tampered_bootstrap_fails_closed_without_credentials(self):
        (self.game/'mods/rusted-bootstrap.jar').write_bytes(b'MODIFIED')
        result=self.invoke();self.assertEqual(result.returncode,1);self.assertNotIn('RUSTED_BOOTSTRAP_JAVA',result.stdout)

    def test_forced_launcher_stop_also_ends_native_child(self):
        self.env['RUSTED_TEST_NATIVE_HOLD']='FIXTURE_ONLY'
        process=subprocess.Popen(self.command(),stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,env=self.env)
        lines=queue.Queue()
        def read():
            for line in process.stdout:lines.put(line)
        threading.Thread(target=read,daemon=True).start()
        try:
            deadline=time.monotonic()+20;pid=None
            while time.monotonic()<deadline:
                try:line=lines.get(timeout=.5)
                except queue.Empty:continue
                if line.startswith('TEST_PEER_NATIVE_PID='):pid=line.strip().split('=',1)[1];break
            self.assertIsNotNone(pid)
            process.kill();process.wait(timeout=10)
            deadline=time.monotonic()+10
            while time.monotonic()<deadline:
                result=subprocess.run(([WINE] if WINE else [])+[native_path(PARENT),'--probe',pid],capture_output=True,text=True,env=self.env,timeout=15)
                if 'DEAD' in result.stdout:break
                time.sleep(.2)
            else:self.fail('Native child survived forced launcher shutdown')
        finally:
            if process.poll() is None:process.kill();process.wait(timeout=10)
            process.stdout.close()


if __name__=='__main__':unittest.main()
