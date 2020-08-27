OUTPUT_DIR := build
ASM_DIR := asm
OUTPUT_DIR_KEEP := $(OUTPUT_DIR)/.keep

$(OUTPUT_DIR)/startup.o: $(ASM_DIR)/x86_64/startup.S $(OUTPUT_DIR_KEEP)
	as -c -o $@ $<

$(OUTPUT_DIR_KEEP):
	mkdir -p $(OUTPUT_DIR)
	touch $@
