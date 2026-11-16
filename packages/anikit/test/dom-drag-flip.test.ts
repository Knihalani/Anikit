import { describe, it, expect, beforeAll } from 'vitest';
import { draggable, gesture } from '../src/index';
import { setup, box } from './setup';

beforeAll(setup);

function ptr(target: EventTarget, type: string, x: number, y: number, pointerId = 1) {
  const e = new (globalThis.PointerEvent ?? MouseEvent)(type, { clientX: x, clientY: y, bubbles: true, button: 0, pointerId } as any);
  if ((e as any).pointerId !== pointerId) Object.defineProperty(e, 'pointerId', { value: pointerId });
  target.dispatchEvent(e);
}

describe('draggable()', () => {
  it('a second pointer neither restarts nor moves the drag', () => {
    const el = box();
    const [d] = draggable(el);
    ptr(el, 'pointerdown', 0, 0, 1);
    ptr(window, 'pointermove', 40, 0, 1);
    ptr(el, 'pointerdown', 40, 0, 2);
    ptr(window, 'pointermove', 40, 300, 2);
    expect(d.x).toBe(40);
    expect(d.y).toBe(0);
    ptr(window, 'pointerup', 40, 300, 2); // the other finger lifts: still dragging
    expect(d.isDragging).toBe(true);
    ptr(window, 'pointermove', 60, 0, 1);
    expect(d.x).toBe(60);
    ptr(window, 'pointerup', 60, 0, 1);
    expect(d.isDragging).toBe(false);
    d.kill();
  });

  it('kill() mid-drag ends the drag and ignores later moves', () => {
    const el = box();
    const [d] = draggable(el);
    ptr(el, 'pointerdown', 0, 0);
    ptr(window, 'pointermove', 30, 0);
    d.kill();
    expect(d.isPressed).toBe(false);
    expect(d.isDragging).toBe(false);
    ptr(window, 'pointermove', 90, 0);
    ptr(window, 'pointercancel', 90, 0);
    expect(d.x).toBe(30);
  });

  it('revert() puts back the transform and handle styles it found', () => {
    const el = box({ transform: 'translateX(20px)' });
    const [d] = draggable(el);
    const x0 = d.x;
    ptr(el, 'pointerdown', 0, 0);
    ptr(window, 'pointermove', 50, 10);
    expect(d.x).toBe(x0 + 50);
    d.revert();
    expect(el.style.transform).toBe('translateX(20px)');
    expect(el.style.touchAction).toBe('');
    expect(el.style.cursor).toBe('');
    expect(d.x).toBe(x0);
  });
});

describe('gesture()', () => {
  it('revert() puts back the transform and styles it found', () => {
    const el = box({ transform: 'rotate(10deg)' });
    const [g] = gesture(el);
    ptr(el, 'pointerdown', 0, 0);
    ptr(window, 'pointermove', 30, 0);
    g.revert();
    expect(el.style.transform).toBe('rotate(10deg)');
    expect(el.style.touchAction).toBe('');
    expect(g.isActive).toBe(false);
  });
});
