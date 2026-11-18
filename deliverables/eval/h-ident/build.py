# embeds bird.rhai + body.rhai into spec.json's top-level script (the spec itself lives in spec.src.json)
import json
spec = json.load(open('spec.src.json'))
spec['script'] = (open('bird.rhai').read() + open('body.rhai').read()).splitlines()
json.dump(spec, open('spec.json', 'w'), indent=1)
