#!/usr/bin/env python3
"""Own fake main class checks private launch framing; never loads Minecraft or an account."""
import argparse
import os
from pathlib import Path
import struct
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[1]
TOKEN='TEST_ONLY_NOT_A_REAL_SESSION_TOKEN'
SOURCE='''package net.minecraft.client.main;
public final class Main { public static void main(String[] args) {
 if(args.length!=3 || !args[0].equals("TEST_ONLY_NOT_A_REAL_SESSION_TOKEN") || !args[1].equals("O'Neil cuivre é #1")) throw new AssertionError();
 if(new java.io.File(args[2]).exists()) throw new AssertionError("session file was not deleted");
 System.out.println("OWN_JAVA_ENTRY_FIXTURE_OK");
}}'''

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--java',required=True);args=parser.parse_args()
 with tempfile.TemporaryDirectory(prefix="Rusted Java O'Neil # ") as temp:
  root=Path(temp);source=root/'Main.java';source.write_text(SOURCE);classes=root/'classes';classes.mkdir()
  subprocess.run(['javac','--release','17','-proc:none','-d',str(classes),str(source)],check=True)
  frame=root/'private-session.bin'
  def string(value):
   b=value.encode();return struct.pack('>I',len(b))+b
  frame.write_bytes(string('net.minecraft.client.main.Main')+struct.pack('>I',3)+b''.join(string(s) for s in [TOKEN,"O'Neil cuivre é #1",str(frame)]))
  frame.chmod(0o600)
  command=[str(args.java),'-cp',os.pathsep.join([str(ROOT/'build/launcher-entry.jar'),str(classes)]),'dev.rusted.launcher.Entrypoint',str(frame)]
  assert TOKEN not in ' '.join(command)
  result=subprocess.run(command,capture_output=True,text=True,timeout=20)
  assert result.returncode==0,result.stderr
  assert 'OWN_JAVA_ENTRY_FIXTURE_OK' in result.stdout
  assert TOKEN not in result.stdout+result.stderr and not frame.exists()
  print('Private Java frame: Unicode arguments preserved, file deleted before fake main, session absent from process argv/logs.')


if __name__=='__main__':main()
