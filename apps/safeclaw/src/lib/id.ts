import { customAlphabet } from "nanoid";

const code = customAlphabet("0123456789");
const nanoid = customAlphabet("abcdefghijklmnopqrstuvwxyz0123456789");

export const getCode = async (length: number = 6) => {
	return await code(length);
};

export const getId = async (length: number = 32) => {
	return await nanoid(length);
};

export const getIdSync = (length: number = 32) => {
	return nanoid(length);
};
