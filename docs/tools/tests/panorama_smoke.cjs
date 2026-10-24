// Isolated DOM-shaped unit harness. No browser, URL navigation, networking, or screenshots.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const root=path.resolve(__dirname, '../..');
class Element {
 constructor(tag='div'){this.localName=tag;this.attrs={};this.children=[];this.events={};this._html='';this.textContent='';this.dataset={};this.parent=null;this.value='';this.style={};this.disabled=false;this.classList={toggle:(name,on)=>{let set=new Set((this.attrs.class||'').split(' ').filter(Boolean));on?set.add(name):set.delete(name);this.attrs.class=[...set].join(' ');}};}
 setAttribute(name,value){this.attrs[name]=String(value);if(name.startsWith('data-'))this.dataset[name.slice(5).replace(/-([a-z])/g,(_,c)=>c.toUpperCase())]=String(value);}
 getAttribute(name){return this.attrs[name]??null;}
 append(...nodes){for(const n of nodes){n.parent=this;this.children.push(n);}}
 remove(){if(this.parent)this.parent.children=this.parent.children.filter(c=>c!==this);}
 replaceChildren(...nodes){this.children=[];this.append(...nodes);}
 addEventListener(type,fn){(this.events[type]??=[]).push(fn);}
 dispatch(type,extra={}){let e={target:this,key:'',preventDefault(){},...extra};for(const fn of this.events[type]||[])fn(e);}
 set innerHTML(value){this._html=String(value);this.children=[];for(const m of this._html.matchAll(/<(button|a|input|h2)\b([^>]*)>/g)){const el=new Element(m[1]);for(const a of m[2].matchAll(/([\w-]+)="([^"]*)"/g))el.setAttribute(a[1],a[2]);el.disabled=/\bdisabled(?:\s|$)/.test(m[2]);this.append(el);}}
 get innerHTML(){return this._html;}
 querySelectorAll(selector){const all=[];function walk(n){for(const c of n.children){if(selector==='button'&&c.localName==='button'||selector==='[data-node]'&&c.attrs['data-node']!==undefined)all.push(c);walk(c);}}walk(this);return all;}
 querySelector(selector){return this.children.find(c=>c.localName===selector)||new Element(selector);}
}
function harness(dataset,script,hash=''){
 const ids=new Map();const document={getElementById(id){if(!ids.has(id))ids.set(id,new Element());return ids.get(id);},createElementNS(ns,tag){return new Element(tag);},documentElement:new Element('html'),title:''};
 document.getElementById('catalog').textContent=JSON.stringify(dataset);
 const window=new Element('window');window.print=()=>{};
 const location={hash};const history={replaceState(_a,_b,newHash){location.hash=newHash;}};
 const storage=new Map();const localStorage={getItem:k=>storage.get(k)||null,setItem:(k,v)=>storage.set(k,v)};
 const context={document,window,location,history,localStorage,console};vm.runInNewContext(script,context,{timeout:4000});return {ids,document,window,location,storage};
}
const plan=JSON.parse(fs.readFileSync(path.join(root,'catalog/implementation-plan.json'))),routes=JSON.parse(fs.readFileSync(path.join(root,'catalog/routes.json')));
const views=fs.readdirSync(path.join(root,'catalog/route-views')).filter(x=>x.endsWith('.json')).map(x=>JSON.parse(fs.readFileSync(path.join(root,'catalog/route-views',x))));
const script=fs.readFileSync(path.join(root,'tools/templates/route-panorama.js'),'utf8');
let profiles=0,graphs=0,scenarios=0,nodes=0;
for(const view of views){
 const h=harness({view,plan,routes},script,'#bad-%hash'),$=id=>h.document.getElementById(id);
 for(const profile of view.profiles){
  const button=$('profiles').querySelectorAll('button').find(b=>b.dataset.profile===profile.id);assert(button);button.dispatch('click');
  assert.equal($('profile-name').textContent,profile.name);assert.equal(h.location.hash,'#'+encodeURIComponent(profile.id));
  assert($('case-title').textContent);
  for(const risk of profile.risks){for(const id of risk.task_ids)assert($('risk-rows').innerHTML.includes('#task-'+id));for(const id of risk.problem_ids)assert($('risk-rows').innerHTML.includes(id));}
  for(const step of profile.build_steps)assert($('build-steps').innerHTML.includes(step.text.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;')));
 assert($('model-cards').innerHTML.includes(profile.detail.models[0].title));
  for(const filter of [...$('graph-filters').querySelectorAll('button')]){
   if(filter.disabled)continue;filter.dispatch('click');const rendered=$('diagram').querySelectorAll('[data-node]');assert.equal(rendered.length,profile.detail.graph.nodes.length);
   for(const n of rendered){n.dispatch('keydown',{key:'Enter'});assert($('node-detail').innerHTML.includes(profile.detail.graph.nodes.find(x=>x.id===n.dataset.node).title.replace(/&/g,'&amp;')));nodes++;}graphs++;
  }
  for(const sc of profile.detail.sequences.scenarios){$('scenarios').querySelectorAll('button').find(b=>b.dataset.scenario===sc.id).dispatch('click');assert.equal($('scenario-title').textContent,sc.label);scenarios++;}
  profiles++;
 }
 const last=view.profiles.at(-1),deep=harness({view,plan,routes},script,'#'+last.id);assert.equal(deep.document.getElementById('profile-name').textContent,last.name);
 h.location.hash='#'+view.profiles[0].id;h.window.dispatch('hashchange');assert.equal($('profile-name').textContent,view.profiles[0].name);
 if(view.slug==='extensions-architecture'){
  for(const category of ['工程服务','诊断增强','语言后端','全部']){$('categories').querySelectorAll('button').find(b=>b.dataset.category===category).dispatch('click');assert($('profiles').querySelectorAll('button').length>0);}
 }
 $('theme').dispatch('click');assert.equal(h.document.documentElement.dataset.theme,'dark');$('theme').dispatch('click');assert.equal(h.document.documentElement.dataset.theme,'light');
}
console.log(JSON.stringify({method:'isolated DOM-shaped unit execution; not a browser',profiles,graph_filters:graphs,sequence_scenarios:scenarios,node_keyboard_selections:nodes,deep_links:'passed',extension_categories:'passed',theme:'passed',network:false}));
