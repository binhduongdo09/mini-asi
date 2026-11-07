const E = require('./vnkey.js');
const type = (s, mode='telex') => { let out=''; for (const ch of s) { const m = out.match(/[A-Za-zÀ-ỹĐđ]+$/); const w = m?m[0]:''; out = out.slice(0,out.length-w.length) + E.apply(w,ch,mode); } return out; };
const cases = [
 ['vieejt','việt'],['vieetj','việt'],['tieengs','tiếng'],['nguwowif','người'],['nguowif','người'],['dduowcj','được'],['hoaf','hòa'],['hoafn','hoàn'],
 ['quas','quá'],['gif','gì'],['muaw','mưa'],['hoawcj','hoặc'],['khuyeen','khuyên'],['thuyr','thủy'],['ddi','đi'],['chaof','chào'],
 ['Vieetj Nam','Việt Nam'],['tooi','tôi'],['laf','là'],['mootj','một'],['uw','ư'],['w','ư'],['tw','tư'],['nhuwngx','những'],['ddepj','đẹp'],
 ['cuar','của'],['giaf','già'],['gios','gió'],['thuowng','thương'],['ruouwj','rượu'],['toans','toán'],['toanss','toans'],['aaa','aa'],
 ['xin chaof cacs banj','xin chào các bạn'],['Hoom nay troiwf ddepj quas!','Hôm nay trời đẹp quá!'],['ngayf mai 9h goij khachs','ngày mai 9h gọi khách'],
 ['thuws 6 noopj baos caos','thứ 6 nộp báo cáo'],['DDaf Naüng',''],['tieengs Anh','tiếng Anh'],['khoong','không'],['ddaauf','đầu'],['nuwowcs','nước'],['huwowngs','hướng'],
 ['oiw','ơi'],['xem phim','xem phim'],['google','gôgle'],['hoir','hỏi'],['quyeenr','quyển'],['giuwax','giữa'],['vaof','vào'],['nhieeuf','nhiều'],
 ['dduwocj','được'],['nuwocs','nước'],['thuwong','thương'],['chuwong','chương'],['tuwongr','tưởng'],['luwongj','lượng'],['huwongs','hướng'],
 ['nguwoif','người'],['muwoif','mười'],['huwou','hươu'],['ruwouj','rượu'],['buwoms','bướm'],['cuwois','cưới'],['tuwoi','tươi'],
 ['duwocjd','được'],['dangd','đang'],['gifa','già'],['gisa','giá'],['toasn','toán'],['tieensg','tiếng'],['vietej','việt'],
 ['developer','developer'],['follow','follow'],['david','david'],
];
let fail=0;
for (const [i,exp] of cases) { if (!exp) continue; const got=type(i); if (got!==exp) { fail++; console.log('TELEX FAIL', i, '=>', got, 'expected', exp); } }
const vni = [['vie65t','việt'],['d9i','đi'],['nguo7i2','người'],['chao2','chào'],['to6i','tôi'],['mu7a','mưa'],['hoa8c5','hoặc'],['d9u7o7c5','được']];
for (const [i,exp] of vni) { const got=type(i,'vni'); if (got!==exp) { fail++; console.log('VNI FAIL', i, '=>', got, 'expected', exp); } }
console.log(fail ? fail+' failures' : 'ALL PASS', cases.length + vni.length);
