'use strict';
(()=>{
 const data=JSON.parse(document.getElementById('catalog').textContent),view=data.view,$=id=>document.getElementById(id),NS='http://www.w3.org/2000/svg';
 const esc=v=>String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
 const taskMap=new Map(data.plan.tasks.map(t=>[t.id,t])),routeMap=new Map(data.routes.routes.map(r=>[r.id,r]));
 const status={not_started:'未开始',in_progress:'进行中',blocked:'受阻',implemented:'已实现'},verification={missing:'缺少证据',running:'验证中',failed:'验证失败',passed:'验证通过',stale:'证据过期'};
 let profile=view.profiles[0],graphFlow='all',scenarioId='call',nodeId='plan',category='全部';
 const filters=[['all','全部'],['build','构建与生成'],['runtime','运行与数据'],['host','反向宿主'],['lifecycle','生命周期'],['delivery','交付验证']];
 function svg(tag,attrs={},text){const el=document.createElementNS(NS,tag);for(const [k,v] of Object.entries(attrs))el.setAttribute(k,String(v));if(text!==undefined)el.textContent=text;return el;}
 function text(parent,x,y,value,cls,more={}){parent.append(svg('text',{x,y,class:cls,...more},value));}
 function marker(root,id,color){const defs=svg('defs'),m=svg('marker',{id,viewBox:'0 0 8 8',refX:7,refY:4,markerWidth:7,markerHeight:7,orient:'auto-start-reverse'});m.append(svg('path',{d:'M1 1 L7 4 L1 7',fill:'none',stroke:color,'stroke-width':1.3}));defs.append(m);root.append(defs);}
 function wrap(value,limit,max=2){const lines=[];let s='',count=0;for(const c of String(value)){const n=c.codePointAt(0)>255?2:1;if(count+n>limit){lines.push(s);s='';count=0;}s+=c;count+=n;}if(s)lines.push(s);if(lines.length>max){lines.length=max;lines[max-1]=lines[max-1].slice(0,-1)+'…';}return lines;}
 const li=values=>values.map(v=>`<li>${esc(v)}</li>`).join('');
 function getHash(){let id='';try{id=decodeURIComponent(location.hash.slice(1));}catch(_){}return view.profiles.find(p=>p.id===id);}
 function renderDetail(){const n=profile.detail.graph.nodes.find(n=>n.id===nodeId)||profile.detail.graph.nodes[0];$('node-detail').innerHTML=`<div><span class="mini">${esc(n.kicker)}</span><h3>${esc(n.title)}</h3><p>${esc(n.summary)}</p></div><div><span class="mini">承担的责任 / 输出</span><ul>${li(n.does)}</ul></div><div><span class="mini">边界与验收</span><ul>${li(n.constraints)}</ul></div>`;}
 function renderGraph(){
  const g=profile.detail.graph,root=$('diagram');root.replaceChildren();root.setAttribute('viewBox',`0 0 ${g.width} ${g.height}`);marker(root,'map-arrow','var(--edge)');
  root.append(svg('title',{},profile.name+' · 完整架构'),svg('desc',{},'分为准备/构建与运行两个阶段，颜色表示责任归属；所有节点和关系默认可见。'));
  for(const b of g.bands){root.append(svg('rect',{x:b.x,y:b.y,width:b.w,height:b.h,rx:15,class:'map-band'}));text(root,b.x+24,b.y+32,b.label,'map-label');}
  for(const b of (g.subbands||[])){root.append(svg('rect',{x:b.x,y:b.y,width:b.w,height:b.h,rx:12,class:'map-subband'}));text(root,b.x+17,b.y+24,b.label,'map-label');}
  const byId=new Map(g.nodes.map(n=>[n.id,n])),active=new Set(graphFlow==='all'?g.nodes.map(n=>n.id):[]);
  const selectedEdges=g.edges.filter(e=>graphFlow==='all'||e.group===graphFlow);selectedEdges.forEach(e=>{active.add(e.a);active.add(e.b);});
  for(const e of g.edges){
   const a=byId.get(e.a),b=byId.get(e.b),right=b.x>a.x;
   const x1=right?a.x+a.w:a.x,x2=right?b.x:b.x+b.w,y1=a.y+a.h/2,y2=b.y+b.h/2;
   const path=e.d||`M${x1} ${y1} H${(x1+x2)/2} V${y2} H${x2}`;
   const group=svg('g',{class:graphFlow==='all'||e.group===graphFlow?'':'dimmed'});
   group.append(svg('path',{d:path,class:'edge '+(e.style==='control'||e.style==='return'?'control':'')+(e.group==='host'?' async':''),'marker-end':'url(#map-arrow)'}));
   if(e.label)text(group,e.lx??(x1+x2)/2,e.ly??y1-14,e.label,'edge-label',{'text-anchor':'middle'});
   root.append(group);
  }
  for(const n of g.nodes){
   const classes='graph-node '+(n.kind==='ui'?'swift':n.kind)+(active.has(n.id)?'':' dimmed')+(nodeId===n.id?' selected':'');
   const el=svg('g',{class:classes,'data-node':n.id,role:'button',tabindex:0,'aria-label':n.title+'，查看责任与约束','aria-pressed':String(nodeId===n.id)});
   el.append(svg('title',{},n.summary),svg('rect',{x:n.x,y:n.y,width:n.w,height:n.h,class:'box'}),svg('rect',{x:n.x+14,y:n.y+16,width:3,height:n.h-32,rx:2,class:'bar'}));
   text(el,n.x+28,n.y+23,n.kicker,'node-kicker');const limit=Math.floor((n.w-44)/7.2),titles=wrap(n.title,limit,2);titles.forEach((s,i)=>text(el,n.x+28,n.y+48+i*19,s,'node-title'));
   const desc=wrap(n.lines.join(' · '),Math.floor((n.w-44)/5.8),2);desc.forEach((s,i)=>text(el,n.x+28,n.y+(titles.length===2?87:78)+i*14,s,'node-copy'));
   function select(){nodeId=n.id;renderDetail();root.querySelectorAll('[data-node]').forEach(e=>{const yes=e.getAttribute('data-node')===nodeId;e.classList.toggle('selected',yes);e.setAttribute('aria-pressed',String(yes));});}
   el.addEventListener('click',select);el.addEventListener('keydown',e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();select();}});root.append(el);
  }
  $('diagram-count').textContent=`${g.nodes.length} 个机制节点 · ${g.edges.length} 条明确关系`;
  renderDetail();
 }
 function renderGraphFilters(){
  $('graph-filters').innerHTML=filters.map(([id,label])=>`<button type="button" class="tab" data-flow="${id}" aria-pressed="${graphFlow===id}" ${id!=='all'&&!profile.detail.graph.edges.some(e=>e.group===id)?'disabled title="此配置不生成该链路"':''}>${label}</button>`).join('');
  $('graph-filters').querySelectorAll('button').forEach(b=>b.addEventListener('click',()=>{if(b.disabled)return;graphFlow=b.dataset.flow;$('graph-filters').querySelectorAll('button').forEach(x=>x.setAttribute('aria-pressed',String(x===b)));renderGraph();}));
 }
 function renderSequence(){
  const seq=profile.detail.sequences,s=seq.scenarios.find(x=>x.id===scenarioId)||seq.scenarios[0];scenarioId=s.id;
  $('scenarios').innerHTML=seq.scenarios.map(x=>`<button type="button" class="tab" data-scenario="${esc(x.id)}" aria-pressed="${x.id===scenarioId}">${esc(x.label)}</button>`).join('');
  $('scenarios').querySelectorAll('button').forEach(b=>b.addEventListener('click',()=>{scenarioId=b.dataset.scenario;renderSequence();}));
  const root=$('sequence');root.replaceChildren();const height=135+s.events.length*72;root.setAttribute('viewBox',`0 0 1240 ${height}`);marker(root,'seq-arrow','var(--accent)');
  root.append(svg('title',{},s.label),svg('desc',{},s.events.map(x=>x.text).join('；')));
  const xs=seq.actors.map((_,i)=>160+i*900/(seq.actors.length-1));
  seq.actors.forEach((name,i)=>{root.append(svg('rect',{x:xs[i]-135,y:16,width:270,height:61,rx:10,class:'seq-head'}));wrap(name,32,2).forEach((t,j)=>text(root,xs[i],41+j*19,t,'seq-owner',{'text-anchor':'middle'}));root.append(svg('path',{d:`M${xs[i]} 86 V${height-24}`,class:'seq-lane'}));});
  s.events.forEach((event,i)=>{const y=127+i*72,x1=xs[event.from_lane],x2=xs[event.to_lane];text(root,14,y+4,String(i+1).padStart(2,'0'),'seq-number');
   if(event.from_lane===event.to_lane){const w=235;root.append(svg('rect',{x:x1-w/2,y:y-24,width:w,height:49,rx:8,class:'seq-self'}));wrap(event.text,34,2).forEach((t,j)=>text(root,x1,y-4+j*16,t,'seq-small',{'text-anchor':'middle'}));}
   else{const dir=x2>x1?1:-1;root.append(svg('path',{d:`M${x1+dir*7} ${y} H${x2-dir*7}`,class:'seq-message '+event.kind,'marker-end':'url(#seq-arrow)'}));wrap(event.text,Math.max(34,Math.floor((Math.abs(x2-x1)-12)/6.5)),2).forEach((t,j,a)=>text(root,(x1+x2)/2,y-13-(a.length-1-j)*17,t,'seq-text',{'text-anchor':'middle'}));}
  });
  $('scenario-title').textContent=s.label;
  $('runtime-notes').innerHTML=s.notes.map((n,i)=>`<article><h3>${i===0?'协议范围':'关键约束'}</h3><p>${esc(n)}</p></article>`).join('')+'<article><h3>箭头不推导能力</h3><p>虚线表示结果/完成，紫色表示投递；具体同步、异步与线程条件以当前契约为准。</p></article>';
 }
 function taskLinks(ids){return ids.map(id=>`<a href="implementation-progress.html#task-${esc(id)}">${esc(id)}</a>`).join(' / ');}
 function renderContent(){
  const d=profile.detail;
  $('graph-intro').textContent=d.family==='service'?'准备期规则与受管输出、执行期请求与提交责任分开展示；默认查看全部关系。':'构建组件生成并交付候选；运行组件承担真实调用。完整关系默认可见，筛选只突出相关链路。';
  $('graph-foot').innerHTML=d.family==='service'?'<strong>实线：</strong>输入、输出或消息；<strong>虚线：</strong>调度、授权或清理。组件不接管宿主的最终验证与提交。':'<strong>实线：</strong>声明、产物或调用；<strong>虚线：</strong>配置、调度或生命周期控制。构建工具不进入应用运行链；依赖关系不规定编译器内部阶段顺序。';
  $('build-title').textContent=d.family==='service'?'从请求计划到受管输出':'从声明到真实消费者';
  $('ownership-intro').textContent=d.family==='service'?'用例、staging、后台任务与事件订阅分别有 owner；关闭 UI 不等于任务停止。':'页面观察、业务对象、在途工作与回调注册分别有 owner；回收时间不能替代停止确认。';
  $('profile-name').textContent=profile.name;$('profile-kind').textContent=profile.kind;$('profile-summary').textContent=profile.summary;$('choices').innerHTML=li(profile.choices);
  $('roles').innerHTML=d.roles.map(r=>`<article class="panel ${esc(r.kind)}"><small>${esc(r.kind.toUpperCase())}</small><h3>${esc(r.title)}</h3><p>${esc(r.body)}</p></article>`).join('');
  const build=profile.build_steps;
  $('build-steps').innerHTML=build.map((s,i)=>`<article><h3>${esc(s.title)}</h3><p>${esc(s.text)}</p></article>`).join('');$('build-steps').style.gridTemplateColumns=`repeat(${build.length},minmax(0,1fr))`;
  // The responsive stylesheet overrides this desktop hint below 900px.
  for(const [id,values] of [['generated',profile.generated],['handwritten',profile.handwritten],['delivery-steps',profile.delivery],['gate-list',profile.gates]])$(id).innerHTML=li(values);
  $('example-title').textContent=d.example.title;$('example-language').textContent=d.example.language;$('example-code').textContent=d.example.code;$('example-note').textContent=d.example.note;
  $('case-title').textContent=d.case_study.title;$('case-trigger').textContent=d.case_study.trigger;
  $('case-detail').innerHTML=[['交错路径',d.case_study.trace],['期望结果',d.case_study.expected],['如何验证',d.case_study.oracle]].map(([t,p])=>`<article><h3>${esc(t)}</h3><p>${esc(p)}</p></article>`).join('');
  $('model-cards').innerHTML=d.models.map(m=>`<article class="panel model-card"><h3>${esc(m.title)}</h3><div class="model-path">${m.path.map((t,i)=>(i?'<div class="arrow" aria-hidden="true">↓</div>':'')+`<div>${esc(t)}</div>`).join('')}</div><p>${esc(m.note)}</p></article>`).join('');
  $('models-title').textContent=d.family==='service'?'请求、产物与错误边界':'类型、数据与错误如何跨界';
  $('owners').innerHTML=d.owners.map(o=>`<div class="level-${o.depth}"><strong>${esc(o.title)}</strong><span>${esc(o.detail)}</span></div>`).join('');
  $('states').innerHTML=d.states.map(s=>`<div><strong>${esc(s.name)}</strong><span>${esc(s.rule)}</span></div>`).join('');
  $('thread-rules').innerHTML=d.thread_rules.map((s,i)=>`<article class="panel"><h3>${['执行环境','线程与方向','资源与共享','重入与完成'][i]||'运行约束'}</h3><p>${esc(s)}</p></article>`).join('');
  $('module-rows').innerHTML=d.modules.map(m=>`<tr><td>${esc(m.module)}</td><td>${esc(m.io)}</td><td>${esc(m.pattern)}</td><td>${esc(m.boundary)}</td></tr>`).join('');
  $('risk-rows').innerHTML=profile.risks.map(r=>`<tr><td><strong>${esc(r.problem)}</strong><p class="hint">${r.problem_ids.map(id=>`<a href="../validation/04-problem-catalog.md">${esc(id)}</a>`).join(' / ')}</p></td><td>${esc(r.solution)}</td><td>${esc(r.example)}</td><td>${taskLinks(r.task_ids)}<p class="hint">需该配置的用例证据</p></td></tr>`).join('');
  $('route-list').innerHTML=profile.route_ids.length?profile.route_ids.map(id=>`<span class="badge">${esc(id)} · ${esc(routeMap.get(id).stage)}</span>`).join(''):'<span class="badge warn">独立扩展；不增加主线路线</span>';
  $('task-list').innerHTML=profile.task_ids.map(id=>{const t=taskMap.get(id);return `<article class="panel task-card"><a href="implementation-progress.html#task-${esc(id)}">${esc(id)} ↗</a><h3>${esc(t.title)}</h3><span class="badge">${esc(status[t.status])}</span> <span class="badge">${esc(verification[t.verification])}</span><p>${esc(t.start_phase)} 启动 / ${esc(t.phase)} 验收</p></article>`;}).join('');
 }
 function renderProfiles(){
  const categories=['全部',...new Set(view.profiles.map(p=>p.detail.category))];$('categories').innerHTML=view.slug==='extensions-architecture'?categories.map(c=>`<button class="tab" type="button" data-category="${esc(c)}" aria-pressed="${c===category}">${esc(c)}</button>`).join(''):'';
  $('categories').querySelectorAll('button').forEach(b=>b.addEventListener('click',()=>{category=b.dataset.category;renderProfiles();if(category!=='全部'&&profile.detail.category!==category)selectProfile(view.profiles.find(p=>p.detail.category===category));}));
  $('profiles').innerHTML=view.profiles.filter(p=>category==='全部'||p.detail.category===category).map(p=>`<button type="button" class="profile-choice" data-profile="${esc(p.id)}" aria-pressed="${profile.id===p.id}"><b>${esc(p.name)}</b><span>${esc(p.kind)}</span></button>`).join('');
  $('profiles').querySelectorAll('button').forEach(b=>b.addEventListener('click',()=>selectProfile(view.profiles.find(p=>p.id===b.dataset.profile))));
 }
 function selectProfile(next,update=true){profile=next;nodeId='plan';if(graphFlow!=='all'&&!profile.detail.graph.edges.some(e=>e.group===graphFlow))graphFlow='all';renderGraphFilters();renderProfiles();renderContent();renderGraph();renderSequence();if(update){try{history.replaceState(null,'','#'+encodeURIComponent(profile.id));}catch(_){}}}
 $('page-title').textContent=view.title;$('summary').textContent=view.summary;$('eyebrow').textContent=view.eyebrow;document.title=view.title+' · MadB';
 $('catalog-stamp').textContent=`设计基线 ${data.plan.baseline} · ${view.profiles.length} 种配置 · 实施与认证状态来自共享目录`;
 const sources=[...view.sources];for(const [label,path] of [['模块与端口','architecture/02-modules-and-ports.md'],['生命周期与并发','architecture/05-lifecycle-and-concurrency.md'],['代码整洁与维护','engineering/02-clean-code.md']])if(!sources.some(s=>s.path===path))sources.push({label,path});
 $('source-links').innerHTML=sources.map(s=>`<a href="${esc(s.url||'../'+s.path)}"${s.url?' target="_blank" rel="noopener noreferrer"':''}>${esc(s.label)} ↗</a>`).join('');
 $('footer-note').textContent='MadB · 路线专题 · 图形解释不替代精确配置认证';
 let dark=false;try{dark=localStorage.getItem('madb-theme')==='dark';}catch(_){}
 function applyTheme(){document.documentElement.dataset.theme=dark?'dark':'light';$('theme').textContent=dark?'浅色':'深色';$('theme').setAttribute('aria-label',dark?'切换浅色主题':'切换深色主题');}applyTheme();
 $('theme').addEventListener('click',()=>{dark=!dark;applyTheme();try{localStorage.setItem('madb-theme',dark?'dark':'light');}catch(_){}});$('print').addEventListener('click',()=>window.print());
 window.addEventListener('hashchange',()=>{const p=getHash();if(p){if(category!=='全部'&&p.detail.category!==category)category='全部';selectProfile(p,false);}});
 renderGraphFilters();selectProfile(getHash()||profile,false);
})();
