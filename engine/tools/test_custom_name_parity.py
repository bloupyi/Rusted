#!/usr/bin/env python3
"""Styled server item names against vanilla ItemStack.getStyledHoverName."""
try:
    from . import lab, oracle_harness as harness
except ImportError:
    import lab, oracle_harness as harness

CASES = [
    '{"text":"Copper"}',
    '{"text":"Copper","italic":0b,"color":"gold"}',
    '{"text":"Copper","bold":1b,"extra":[{"text":" blade","italic":0b,"color":"aqua"}]}',
    '{"text":"Copper","italic":0b,"extra":[{"text":" blade","italic":1b}]}',
    '{"text":"Copper","underlined":1b,"strikethrough":1b}',
    '"Anvil name"',
]

def cases(build):
    lines = ['{id:"minecraft:stick",count:1,components:{"minecraft:' + kind + '":' + name + '}}'
             for kind in ['custom_name', 'item_name'] for name in CASES]
    path = build / 'cases.txt'
    path.write_text('\n'.join(lines) + '\n')
    return path, len(lines)

if __name__ == '__main__':
    harness.differential('custom-name-parity', 'VanillaCustomNameOracle', 'custom_name_fixture', cases,
        'Custom/item names: default italics, explicit false, nested spans, colours and decoration',
        sources=[lab.ROOT/'crates/horizon-client/src/item_tooltip.rs',
                 lab.ROOT/'crates/horizon-items/src/item_data.rs'], expected_lines=12)
