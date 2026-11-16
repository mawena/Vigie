// Vigie : dates locales, infobulle des courbes, rafraîchissement toutes les minutes.
(function () {
  'use strict';
  var deux = function (n) { return (n < 10 ? '0' : '') + n; };
  function dateCourte(t) {
    var d = new Date(t * 1000);
    return deux(d.getDate()) + '/' + deux(d.getMonth() + 1) + ' ' + deux(d.getHours()) + ':' + deux(d.getMinutes());
  }
  function relatif(t) {
    var s = Math.max(0, Math.round(Date.now() / 1000 - t));
    if (s < 60) return 'il y a ' + s + ' s';
    if (s < 3600) return 'il y a ' + Math.round(s / 60) + ' min';
    return dateCourte(t);
  }
  function dates() {
    document.querySelectorAll('[data-t]').forEach(function (e) {
      var t = Number(e.getAttribute('data-t'));
      e.textContent = e.hasAttribute('data-relatif') ? relatif(t) : dateCourte(t);
    });
  }
  function nombre(v, unite) {
    var d = Math.abs(v) < 10 ? 2 : 0;
    return v.toFixed(d).replace('.', ',') + (unite ? ' ' + unite : '');
  }

  // Viseur + infobulle : la zone de survol est toute la courbe, le point le plus proche est retenu.
  document.querySelectorAll('.courbe svg').forEach(function (svg) {
    var points = JSON.parse(svg.getAttribute('data-points') || '[]');
    if (points.length < 2) return;
    var ns = 'http://www.w3.org/2000/svg';
    var vb = svg.viewBox.baseVal;
    var g = +svg.dataset.g, d = +svg.dataset.d, haut = +svg.dataset.haut, bas = +svg.dataset.bas, ymax = +svg.dataset.ymax;
    var unite = svg.dataset.unite;
    var t0 = points[0][0], t1 = points[points.length - 1][0];
    var x = function (t) { return g + (t - t0) / Math.max(1, t1 - t0) * (vb.width - g - d); };
    var y = function (v) { return haut + (1 - Math.min(1, Math.max(0, v / ymax))) * (vb.height - haut - bas); };
    var viseur = document.createElementNS(ns, 'line');
    viseur.setAttribute('class', 'viseur');
    viseur.setAttribute('y1', haut); viseur.setAttribute('y2', vb.height - bas);
    var point = document.createElementNS(ns, 'circle');
    point.setAttribute('class', 'point'); point.setAttribute('r', 5);
    var bulle = document.createElement('div');
    bulle.className = 'bulle'; bulle.hidden = true;
    svg.parentNode.appendChild(bulle);
    function cacher() { viseur.remove(); point.remove(); bulle.hidden = true; }
    function montrer(ev) {
      var r = svg.getBoundingClientRect();
      var vx = (ev.clientX - r.left) / r.width * vb.width;
      var meilleur = points[0], ecart = Infinity;
      points.forEach(function (p) { var e = Math.abs(x(p[0]) - vx); if (e < ecart) { ecart = e; meilleur = p; } });
      var px = x(meilleur[0]), py = y(meilleur[1]);
      viseur.setAttribute('x1', px); viseur.setAttribute('x2', px);
      point.setAttribute('cx', px); point.setAttribute('cy', py);
      svg.appendChild(viseur); svg.appendChild(point);
      bulle.textContent = nombre(meilleur[1], unite) + ' — ' + dateCourte(meilleur[0]);
      bulle.hidden = false;
      var gauche = px / vb.width * r.width;
      bulle.style.left = Math.min(r.width - bulle.offsetWidth / 2, Math.max(bulle.offsetWidth / 2, gauche)) + 'px';
      bulle.style.top = (py / vb.height * r.height - 34) + 'px';
    }
    svg.addEventListener('pointermove', montrer);
    svg.addEventListener('pointerdown', montrer);
    svg.addEventListener('pointerleave', cacher);
  });

  dates();
  setInterval(dates, 15000);
  // Rechargement toutes les minutes, sauf pendant la lecture d'une infobulle.
  setInterval(function () {
    if (!document.querySelector('.bulle:not([hidden])') && !document.hidden) location.reload();
  }, 60000);
})();
