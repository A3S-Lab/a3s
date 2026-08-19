import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { architectureProjects } from './architecture';
import {
  deliveryStages,
  getDeliveryStageCopy,
  getProjectDeliveryStatus,
  statusProjectIds,
  statusVerifiedAt,
} from './ecosystem-status';

describe('ecosystem delivery status', () => {
  test('covers every project in the ecosystem directory', () => {
    assert.deepEqual(
      [...statusProjectIds].sort(),
      architectureProjects.map((project) => project.id).sort(),
    );
  });

  test('uses categorical stages without numeric completion data', () => {
    for (const project of architectureProjects) {
      const chinese = getProjectDeliveryStatus(project.id, 'cn');
      const english = getProjectDeliveryStatus(project.id, 'en');

      assert.ok(deliveryStages.includes(chinese.stage));
      assert.equal(chinese.stage, english.stage);
      assert.equal(chinese.release, english.release);
      assert.ok(chinese.release.length > 0);
      assert.notEqual(chinese.label, english.label);
      assert.notEqual(chinese.description, english.description);
      assert.deepEqual(
        Object.keys(chinese).sort(),
        ['description', 'label', 'release', 'stage'],
      );
    }
  });

  test('defines what every stage means in both languages', () => {
    for (const stage of deliveryStages) {
      const chinese = getDeliveryStageCopy(stage, 'cn');
      const english = getDeliveryStageCopy(stage, 'en');

      assert.ok(chinese.label.length > 0);
      assert.ok(chinese.description.length > 0);
      assert.ok(english.label.length > 0);
      assert.ok(english.description.length > 0);
    }
  });

  test('records the verified status snapshot', () => {
    assert.equal(statusVerifiedAt, '2026-08-19');
    assert.equal(getProjectDeliveryStatus('cli', 'en').release, 'v0.12.1');
    assert.equal(getProjectDeliveryStatus('flow', 'en').release, 'v0.11.0');
    assert.equal(getProjectDeliveryStatus('form', 'en').stage, 'preview');
    assert.equal(getProjectDeliveryStatus('windhole', 'en').stage, 'preview');
    assert.equal(getProjectDeliveryStatus('gui', 'en').stage, 'preview');
    assert.equal(getProjectDeliveryStatus('oci-runtime', 'en').stage, 'experimental');
    assert.equal(getProjectDeliveryStatus('oci-runtime', 'en').release, 'v0.2.0');
    assert.equal(getProjectDeliveryStatus('updater', 'en').stage, 'released');
    assert.equal(getProjectDeliveryStatus('updater', 'en').release, 'v0.3.0');
  });
});
