import * as THREE from 'three';
import type { WeaponLoadout, WeaponStoreKind, WeaponPlacement } from './weapon-loadout';

interface StoreDescriptor {
  kind: WeaponStoreKind;
  placement: WeaponPlacement;
}

export function createWeaponLoadoutVisual(loadout: WeaponLoadout): THREE.Group {
  const group = new THREE.Group();
  group.name = `weapon-loadout:${loadout.id}`;
  group.userData.loadout = {
    id: loadout.id,
    displayName: loadout.displayName,
    effort: loadout.effort,
    totalStores: loadout.totalStores,
    visualizationOnly: true,
  };

  const descriptors = loadout.stores.flatMap((store) =>
    Array.from({ length: store.quantity }, () => ({ kind: store.kind, placement: store.placement }))
  );
  const internal = descriptors.filter((store) => store.placement === 'internal');
  const external = descriptors.filter((store) => store.placement === 'external');

  if (internal.length > 0) {
    const bay = new THREE.Mesh(
      new THREE.BoxGeometry(2.25, 0.09, 0.98),
      new THREE.MeshStandardMaterial({ color: 0x11191c, metalness: 0.72, roughness: 0.36 })
    );
    bay.name = 'open-weapon-bay';
    bay.position.set(0.25, -0.4, 0);
    group.add(bay);
  }

  internal.forEach((store, index) => {
    const row = index % 2 === 0 ? -1 : 1;
    const column = Math.floor(index / 2);
    const missile = createMissile(store, index);
    missile.position.set(-0.42 + column * 0.48, -0.52, row * 0.27);
    group.add(missile);
  });

  external.forEach((store, index) => {
    const side = index % 2 === 0 ? -1 : 1;
    const column = Math.floor(index / 2);
    const pylon = new THREE.Mesh(
      new THREE.BoxGeometry(0.55, 0.2, 0.07),
      new THREE.MeshStandardMaterial({ color: 0x303b3e, metalness: 0.78, roughness: 0.3 })
    );
    pylon.name = `weapon-pylon:${index}`;
    pylon.position.set(0.15 + column * 0.38, -0.21, side * (1.25 + column * 0.25));
    group.add(pylon);

    const missile = createMissile(store, internal.length + index);
    missile.position.set(0.03 + column * 0.38, -0.42, side * (1.25 + column * 0.25));
    group.add(missile);
  });

  group.traverse((object) => {
    if (object instanceof THREE.Mesh) {
      object.castShadow = true;
      object.receiveShadow = true;
    }
  });
  return group;
}

function createMissile(store: StoreDescriptor, index: number): THREE.Group {
  const missile = new THREE.Group();
  missile.name = `weapon-store:${index}:${store.kind}:${store.placement}`;
  missile.userData.weaponStore = { ...store };

  const isMediumRange = store.kind === 'medium-range-aam';
  const length = isMediumRange ? 0.86 : 0.64;
  const radius = isMediumRange ? 0.055 : 0.045;
  const bodyMaterial = new THREE.MeshStandardMaterial({
    color: isMediumRange ? 0xd7dde0 : 0xc3cbd0,
    metalness: 0.62,
    roughness: 0.27,
  });
  const detailMaterial = new THREE.MeshStandardMaterial({
    color: isMediumRange ? 0x6f8790 : 0xd64545,
    metalness: 0.55,
    roughness: 0.32,
  });

  const body = new THREE.Mesh(new THREE.CylinderGeometry(radius, radius, length, 10), bodyMaterial);
  body.name = 'weapon-body';
  body.rotation.z = Math.PI / 2;
  missile.add(body);

  const nose = new THREE.Mesh(new THREE.ConeGeometry(radius, 0.18, 10), detailMaterial);
  nose.name = 'weapon-seeker';
  nose.rotation.z = Math.PI / 2;
  nose.position.x = -length / 2 - 0.09;
  missile.add(nose);

  for (const side of [-1, 1]) {
    const fin = new THREE.Mesh(new THREE.BoxGeometry(0.13, 0.012, 0.14), detailMaterial);
    fin.name = `weapon-fin:${side}`;
    fin.position.set(length * 0.28, 0, side * radius * 0.75);
    missile.add(fin);
  }
  return missile;
}
