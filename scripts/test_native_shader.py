#!/usr/bin/env python3
"""Verify the original optional WGSL pack in a fresh native world, never player data."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import tempfile
import time
from test_horizon_native import NativeGame, ROOT


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=ROOT/'build/rusted-linux-target/release/rusted')
    parser.add_argument('--resources',type=Path,default=ROOT/'build/horizon/source/.local/horizon-client/26.3')
    args=parser.parse_args()
    scratch=Path(tempfile.mkdtemp(prefix='rusted-shader-035-',dir=ROOT/'build/horizon'))
    home=scratch/'game';(home/'shaderpacks').mkdir(parents=True)
    shader=ROOT/'build/Rusted-Soft-Light.hshader';shutil.copyfile(shader,home/'shaderpacks'/shader.name)
    old=os.environ.get('HORIZON_SHADER_PACK');os.environ['HORIZON_SHADER_PACK']=shader.name
    game=NativeGame(args.binary.resolve(),args.resources.resolve(),scratch,home,False)
    record={'shader':'Rusted Soft Light 0.1.0','shader_sha256':hashlib.sha256(shader.read_bytes()).hexdigest(),'account':False,'passed':False}
    try:
        game.wait(lambda s:s.get('screen')=='title')
        game.command('open RustedShaderTest 1 creative')
        state=game.wait(lambda s:s.get('world') is not None and s.get('player') is not None)
        deadline=time.monotonic()+15
        while time.monotonic()<deadline:
            text=(scratch/'console.log').read_text()
            if 'shader pack rusted_soft_light 0.1.0:' in text:break
            time.sleep(.2)
        else:
            record['state_at_failure']=game.command('state')
            record['screenshot_at_failure']=game.screenshot('failure')
            record['stats_at_failure']=game.command('stats')
            raise RuntimeError('Native WGSL pack was not loaded')
        time.sleep(1)
        record.update(passed=True,state=state,screenshot=game.screenshot('world-soft-light'),stats=game.command('stats'))
        game.close()
        text=(scratch/'console.log').read_text()
        assert 'Horizon renderer:' not in text
        adapters=[l for l in text.splitlines() if l.startswith('Rusted renderer:') and any(t in l for t in ['(Vulkan,','(Dx12,','(Gl,','(Metal,'])]
        assert len(adapters)==1,adapters
        assert 'shader pack Rusted-Soft-Light.hshader refused' not in text
        record['renderer_adapter_reports']=adapters
        record['shader_loaded_line']=next(l for l in text.splitlines() if 'shader pack rusted_soft_light 0.1.0:' in l)
    except BaseException:
        record['passed']=False
        if game.process.poll() is None:game.process.kill();game.process.wait()
        raise
    finally:
        if old is None:os.environ.pop('HORIZON_SHADER_PACK',None)
        else:os.environ['HORIZON_SHADER_PACK']=old
        (scratch/'validation.json').write_text(json.dumps(record,indent=2)+'\n')
        (ROOT/'build/horizon/latest-035-shader.json').write_text(json.dumps({'scratch':str(scratch)}))
        print('Native shader validation:',scratch)


if __name__=='__main__':main()
